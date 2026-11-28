//! The commands that make and change a document's sheets and the viewports on them (CON-A11-F1-P1,
//! CON-A11-F1-P2). They go through the same edit log as `element.*`, with the same access check
//! (`model.edit`) and the same claims: a piece another person is working on is refused, and one
//! person's edits land in log order.
//!
//! A sheet is an element record of the sheets category and a viewport one of the viewports
//! category, neither with a shape ([`construct_sheets`]). Each command reads the document's sheets,
//! viewports and views, makes the change with construct-sheets' own types, checks it with the rules
//! of [`construct_sheets::Sheets`] and writes the records it changed. A refusal is the command's
//! result, in construct-sheets' words, and changes nothing. Nothing here reads a clock or a random
//! number.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `sheet.create` | `sheet` (the new ID), `number`, `name`; optional `paper` | A sheet record; and the parameter table when it lacks the standard sheet parameters |
//! | `sheet.set` | `sheet`, and any of `number`, `name`, `paper` | The sheet, its parameter values kept; nothing at all when it already is so |
//! | `sheet.delete` | `sheet` | The sheet and every viewport on it |
//! | `viewport.create` | `viewport` (the new ID), `sheet`, `view`; optional `x`, `y` (the centre on the paper, default the paper's centre), `title` (default on) | A viewport record |
//! | `viewport.set` | `viewport`, and any of `x`, `y`, `title` | The viewport; an axis it is aligned on is refused |
//! | `viewport.align` | `viewport`, and `across` and/or `up`: `{"to": <viewport>, "anchor": "min"\|"centre"\|"max"}` or `null` to take the alignment away | The viewport |
//! | `viewport.delete` | `viewport` | Its pointer goes; refused while another viewport is aligned to it |
//!
//! The values: `number` is text, at most 32 characters, unique in the document without regard to
//! case. `paper` is `{"size": "a0".."a5", "orientation": "landscape"|"portrait"}` (orientation
//! default landscape) or `{"width": <typed text>, "height": <typed text>}` for a custom size from
//! 50 mm to 5 m each; a new sheet is A1 landscape. `x` and `y` are typed text read with
//! construct-params' metric display settings (`300`, `297.5 mm`),
//! measured from the paper's lower left corner. A viewport takes the view's own scale and shows the
//! view's name as its title when `title` is on.
//!
//! Refused by name: a number another sheet has; a view that is already placed (on this sheet or
//! another); a view that is not there; a centre off the paper; paper that would leave a viewport's
//! centre off it; an alignment to itself, to a viewport of another sheet or round a loop; a hand
//! placement on an axis the viewport is aligned on; deleting a viewport another is aligned to.
//! Deleting a view that is placed is refused by `view.delete` ([`placed_refusal`]).
//!
//! The standard sheet parameters (drawn by, checked by, issue date, revision...,
//! [`construct_sheets::STANDARD`]) are defined and bound to the sheets category by the first
//! `sheet.create` of a document, in the same step, and their values are set on a sheet by the
//! ordinary `element.param`, so a title block reads them as it reads any parameter.
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of a sheet or a
//! viewport and `element.set`, `element.move` or `element.delete` of one are refused, naming the
//! command to use.

use crate::cloud::elements::write_record;
use crate::cloud::views::{Shaped, World as ViewWorld, listing_of};
use construct_categories::{SHEETS, VIEWPORTS};
use construct_levels::parse_length;
use construct_model::element::Element;
use construct_params::{Length, Units, Value};
use construct_regen::parameters as records;
use construct_sheets::{
    Alignment, Anchor, Orientation, Paper, PaperSize, SHEET_KIND, Sheet, Sheets, VIEWPORT_KIND,
    ViewFacts, Viewport, with_standard,
};
use kit_core::Id;
use kit_ops::{Change, Claim, Edit};
use kit_store::{Place, Reader, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Every command here.
pub const COMMANDS: [&str; 7] = [
    "sheet.create",
    "sheet.set",
    "sheet.delete",
    "viewport.create",
    "viewport.set",
    "viewport.align",
    "viewport.delete",
];

/// The answer to a command that would make or change a sheet or a viewport as an element.
pub const BY_SHEET_COMMANDS: &str =
    "Sheets and viewports are made and changed by the sheet and viewport commands.";

/// The paper of a sheet made without one.
pub const DEFAULT_PAPER: Paper = Paper::Standard {
    size: PaperSize::A1,
    orientation: Orientation::Landscape,
};

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
    sheet: Option<Id>,
    #[serde(default, with = "kit_ops::id::option")]
    viewport: Option<Id>,
    #[serde(default, with = "kit_ops::id::option")]
    view: Option<Id>,
    number: Option<String>,
    name: Option<String>,
    paper: Option<PaperArgs>,
    x: Option<String>,
    y: Option<String>,
    title: Option<bool>,
    /// The alignment across the paper; `null` takes it away, so `null` is kept apart from none.
    #[serde(default, deserialize_with = "present")]
    across: Option<AlignChange>,
    /// The alignment up the paper.
    #[serde(default, deserialize_with = "present")]
    up: Option<AlignChange>,
}

/// An argument that is there, `null` included.
fn present<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<AlignChange>, D::Error> {
    Option::<AlignArgs>::deserialize(deserializer)
        .map(|align| Some(align.map_or(AlignChange::Away, AlignChange::To)))
}

/// What an alignment argument asks for.
enum AlignChange {
    /// `null`: the alignment is taken away.
    Away,
    /// An alignment to make.
    To(AlignArgs),
}

/// Paper as a person sends it.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PaperArgs {
    size: Option<String>,
    orientation: Option<String>,
    width: Option<String>,
    height: Option<String>,
}

/// An alignment as a person sends it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AlignArgs {
    #[serde(with = "kit_ops::id")]
    to: Id,
    anchor: String,
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
    parse_length(text, &Units::default()).map_err(|e| e.to_string())
}

/// A document as the sheet commands read it: its sheets and viewports, its views, and each
/// record's hash.
pub struct World {
    sheets: Sheets,
    views: construct_views::Views,
    all: BTreeMap<Id, (Id, Element)>,
}

impl World {
    /// The document's sheets and viewports.
    #[must_use]
    pub fn sheets(&self) -> &Sheets {
        &self.sheets
    }

    /// The document's views.
    #[must_use]
    pub fn views(&self) -> &construct_views::Views {
        &self.views
    }

    /// The record of a piece, with its hash.
    fn record(&self, id: &Id) -> Option<&(Id, Element)> {
        self.all.get(id)
    }
}

/// A document's sheets, viewports and views as they are now. The inner error is a record that is
/// not as written, in words.
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
    let sheets = match Sheets::from_records(found.iter().copied()) {
        Ok(sheets) => sheets,
        Err(e) => return Ok(Err(e.to_string())),
    };
    let views = match construct_views::Views::from_records(found.iter().copied()) {
        Ok(views) => views,
        Err(e) => return Ok(Err(e.to_string())),
    };
    Ok(Ok(World { sheets, views, all }))
}

/// A sheet as a page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SheetSummary {
    /// Its permanent ID.
    pub id: String,
    /// Its number.
    pub number: String,
    /// Its name.
    pub name: String,
    /// The paper's key: `a0` to `a5`, or `custom`.
    pub paper: String,
    /// `landscape` or `portrait` for a standard size, `None` for a custom one.
    pub orientation: Option<String>,
    /// The paper's width, in construct-params' stored length text.
    pub width: String,
    /// The paper's height, in the same text.
    pub height: String,
    /// The paper as people read it: `A1 landscape (841 mm by 594 mm)`.
    pub shown: String,
    /// How many viewports are on it.
    pub viewports: usize,
}

fn stored(length: Length) -> String {
    Value::Length(length).stored()
}

/// The summary of one sheet.
#[must_use]
pub fn summary(sheet: &Sheet, sheets: &Sheets) -> SheetSummary {
    let paper = sheet.paper();
    SheetSummary {
        id: sheet.id().to_string(),
        number: sheet.number().to_owned(),
        name: sheet.name().to_owned(),
        paper: paper.key().to_owned(),
        orientation: match paper {
            Paper::Standard { orientation, .. } => Some(orientation.key().to_owned()),
            Paper::Custom { .. } => None,
        },
        width: stored(paper.width()),
        height: stored(paper.height()),
        shown: paper.describe(),
        viewports: sheets.viewports_of(sheet.id()).len(),
    }
}

/// Every sheet of the world, by number (case aside), then by ID.
#[must_use]
pub fn summaries(world: &World) -> Vec<SheetSummary> {
    world
        .sheets
        .ordered()
        .into_iter()
        .map(|sheet| summary(sheet, &world.sheets))
        .collect()
}

/// A rectangle on the paper as a page reads it, each side in stored length text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutlineView {
    /// Left edge.
    pub min_x: String,
    /// Bottom edge.
    pub min_y: String,
    /// Right edge.
    pub max_x: String,
    /// Top edge.
    pub max_y: String,
}

/// An alignment as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AlignView {
    /// The viewport followed.
    pub to: String,
    /// `min`, `centre` or `max`.
    pub anchor: String,
}

impl From<&Alignment> for AlignView {
    fn from(align: &Alignment) -> Self {
        Self {
            to: align.to.to_string(),
            anchor: align.anchor.key().to_owned(),
        }
    }
}

/// A viewport as a page lists it: where the sheet puts it, from the alignments and each view's
/// extent and scale ([`construct_sheets::Sheets::layout`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ViewportView {
    /// Its permanent ID.
    pub id: String,
    /// The view it shows.
    pub view: String,
    /// That view's name, which is its title when [`ViewportView::title`] is on.
    pub view_name: String,
    /// The centre across the paper, alignments worked out, in stored length text.
    pub x: String,
    /// The centre up the paper.
    pub y: String,
    /// The view's scale, the `n` of 1:n.
    pub scale: u32,
    /// Whether the title shows.
    pub title: bool,
    /// What it follows across the paper.
    pub align_x: Option<AlignView>,
    /// What it follows up the paper.
    pub align_y: Option<AlignView>,
    /// Its outline on the paper, or `None` for a view with no extent yet.
    pub outline: Option<OutlineView>,
    /// Whether the whole outline is on the paper.
    pub on_paper: bool,
}

/// One sheet with its viewports.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SheetListing {
    /// The sheet.
    pub sheet: SheetSummary,
    /// Its viewports, by ID.
    pub viewports: Vec<ViewportView>,
    /// The schedules placed on it, each drawn from its live table; the listing of a sheet from
    /// the sheets alone has none, the server adds them ([`crate::cloud::schedules::placed_on`]).
    pub schedules: Vec<crate::cloud::schedules::PlacedSchedule>,
}

/// The extent of a view as far as it is known: a plan's is the box of what it cuts and sees, in
/// whole millimetres, rounded outward as the shapes' boxes are; the other kinds have none yet
/// (their crops and datum extents are later jobs), so they are points on the paper.
fn extent_of(
    views: &ViewWorld,
    id: &Id,
    shaped: &[Shaped],
) -> Result<Option<(Length, Length)>, String> {
    let plan = views
        .views()
        .get(id)
        .is_some_and(|view| view.kind().plan().is_some());
    if !plan {
        return Ok(None);
    }
    let Some(listing) = listing_of(views, id, shaped)? else {
        return Ok(None);
    };
    let shown: Vec<&String> = listing.cut.iter().chain(&listing.seen).collect();
    let mut low: Option<[i64; 2]> = None;
    let mut high: Option<[i64; 2]> = None;
    for one in shaped {
        if !shown.contains(&&one.record.id.to_string()) {
            continue;
        }
        let (min, max) = (one.bounds.min, one.bounds.max);
        let [min_x, min_y] = [min.first(), min.get(1)].map(|v| v.copied().unwrap_or(0));
        let [max_x, max_y] = [max.first(), max.get(1)].map(|v| v.copied().unwrap_or(0));
        low = Some(low.map_or([min_x, min_y], |l| [l[0].min(min_x), l[1].min(min_y)]));
        high = Some(high.map_or([max_x, max_y], |h| [h[0].max(max_x), h[1].max(max_y)]));
    }
    let (Some(low), Some(high)) = (low, high) else {
        return Ok(None);
    };
    let side = |lo: i64, hi: i64| {
        hi.checked_sub(lo)
            .and_then(Length::checked_from_mm)
            .ok_or_else(|| "A plan is too large to be placed on a sheet.".to_owned())
    };
    Ok(Some((side(low[0], high[0])?, side(low[1], high[1])?)))
}

/// One sheet with each viewport placed on its paper, from the document's views and the shapes the
/// model holds. `None` when the document has no such sheet.
///
/// # Errors
/// The crates' words for a record that does not read, or an extent that does not fit.
pub fn listing_of_sheet(
    world: &World,
    views: &ViewWorld,
    id: &Id,
    shaped: &[Shaped],
) -> Result<Option<SheetListing>, String> {
    let Some(sheet) = world.sheets.sheet(id) else {
        return Ok(None);
    };
    let mut facts: BTreeMap<Id, ViewFacts> = BTreeMap::new();
    for viewport in world.sheets.viewports_of(id) {
        let Some(view) = views.views().get(viewport.view()) else {
            return Err(
                construct_sheets::SheetError::UnknownView(viewport.view().clone()).to_string(),
            );
        };
        facts.insert(
            viewport.view().clone(),
            ViewFacts {
                scale: view.scale(),
                extent: extent_of(views, viewport.view(), shaped)?,
            },
        );
    }
    let placed = world
        .sheets
        .layout(id, |view| facts.get(view).copied())
        .map_err(|e| e.to_string())?;
    let mut shown = Vec::new();
    for one in placed {
        let Some(viewport) = world.sheets.viewport(&one.viewport) else {
            continue;
        };
        let view_name = views
            .views()
            .get(&one.view)
            .map(|view| view.name().to_owned())
            .unwrap_or_default();
        shown.push(ViewportView {
            id: one.viewport.to_string(),
            view: one.view.to_string(),
            view_name,
            x: stored(one.x),
            y: stored(one.y),
            scale: one.scale.denominator(),
            title: one.title,
            align_x: viewport.align_x().map(AlignView::from),
            align_y: viewport.align_y().map(AlignView::from),
            outline: one.outline.map(|o| OutlineView {
                min_x: stored(o.min_x),
                min_y: stored(o.min_y),
                max_x: stored(o.max_x),
                max_y: stored(o.max_y),
            }),
            on_paper: one.on_paper,
        });
    }
    Ok(Some(SheetListing {
        sheet: summary(sheet, &world.sheets),
        viewports: shown,
        schedules: Vec::new(),
    }))
}

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

    /// The document's sheets, viewports and views as they are now.
    async fn world(&mut self) -> Result<Result<World, String>, StoreError> {
        read_world(self.txn, self.project, self.document).await
    }

    /// Makes the writes of one command, taking this person's claim on every piece first: another
    /// person's claim on any of them refuses it all.
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
    let world = match run.world().await? {
        Ok(world) => world,
        Err(refusal) => return Ok(Err(refusal)),
    };
    match edit.cmd.as_str() {
        "sheet.create" => create_sheet(&mut run, &world, args).await,
        "sheet.set" => set_sheet(&mut run, &world, args).await,
        "sheet.delete" => delete_sheet(&mut run, &world, args).await,
        "viewport.create" => create_viewport(&mut run, &world, args).await,
        "viewport.set" => set_viewport(&mut run, &world, args).await,
        "viewport.align" => align_viewport(&mut run, &world, args).await,
        "viewport.delete" => delete_viewport(&mut run, &world, args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

fn paper_of(args: &PaperArgs) -> Result<Paper, String> {
    match (&args.size, &args.width, &args.height) {
        (Some(key), None, None) => {
            let size = PaperSize::from_key(key).ok_or_else(|| {
                let sizes: Vec<&str> = PaperSize::ALL.iter().map(|s| s.key()).collect();
                format!("{key:?} is not a paper size: {}.", sizes.join(", "))
            })?;
            let orientation = match args.orientation.as_deref() {
                None => Orientation::Landscape,
                Some(text) => Orientation::from_key(text).ok_or_else(|| {
                    format!("{text:?} is not an orientation: landscape or portrait.")
                })?,
            };
            Ok(Paper::Standard { size, orientation })
        }
        (None, Some(width), Some(height)) if args.orientation.is_none() => Ok(Paper::Custom {
            width: length(width)?,
            height: length(height)?,
        }),
        _ => Err("Paper is a size (and an orientation), or a width and a height.".to_owned()),
    }
}

/// The standard sheet parameters, written into the table when it lacks them: the table record
/// and the hash it replaces, or none when it has them already.
async fn table_write<T: Txn>(
    run: &mut Run<'_, T>,
) -> Result<Result<Option<Write>, String>, StoreError> {
    let context = match records::read_context(run.txn, run.project, run.document).await? {
        Ok(context) => context,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let table = match with_standard(&context.table, &context.categories) {
        Ok(table) => table,
        Err(e) => return Ok(Err(e.to_string())),
    };
    if table == context.table {
        return Ok(Ok(None));
    }
    let before = records::read_element(run.txn, run.project, run.document, &records::table_id())
        .await?
        .map(|(hash, _)| hash);
    Ok(records::table_record(run.document, &table)
        .map(|record| Some(Write::Put(Box::new(record), before))))
}

async fn create_sheet<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let made = (|| {
        let id = need(args.sheet.clone(), "a sheet")?;
        let number = need(args.number.as_deref(), "a number")?;
        let name = need(args.name.as_deref(), "a name")?;
        let paper = match &args.paper {
            Some(paper) => paper_of(paper)?,
            None => DEFAULT_PAPER,
        };
        Sheet::new(id, number, name, paper).map_err(|e| e.to_string())
    })();
    let sheet = match made {
        Ok(sheet) => sheet,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if world.record(sheet.id()).is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let mut sheets = world.sheets.clone();
    if let Err(e) = sheets.add_sheet(sheet.clone()) {
        return Ok(Err(e.to_string()));
    }
    let mut writes = vec![Write::Put(Box::new(sheet.to_record(run.document)), None)];
    match table_write(run).await? {
        Ok(Some(write)) => writes.push(write),
        Ok(None) => {}
        Err(refusal) => return Ok(Err(refusal)),
    }
    run.write_all(writes).await
}

/// The sheet a command is about, with its record and hash.
fn sheet_of(world: &World, id: Option<Id>) -> Result<(&Id, &Element, Sheet), String> {
    let id = need(id, "a sheet")?;
    let missing = || construct_sheets::SheetError::UnknownSheet(id.clone()).to_string();
    let (hash, record) = world.record(&id).ok_or_else(missing)?;
    if record.kind != SHEET_KIND {
        return Err(missing());
    }
    let sheet = Sheet::from_record(record).map_err(|e| e.to_string())?;
    Ok((hash, record, sheet))
}

async fn set_sheet<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, record, old) = match sheet_of(world, args.sheet.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = (|| {
        let mut sheet = old.clone();
        if let Some(number) = &args.number {
            sheet = sheet.with_number(number).map_err(|e| e.to_string())?;
        }
        if let Some(name) = &args.name {
            sheet = sheet.with_name(name).map_err(|e| e.to_string())?;
        }
        if let Some(paper) = &args.paper {
            sheet = sheet
                .with_paper(paper_of(paper)?)
                .map_err(|e| e.to_string())?;
        }
        Ok::<_, String>(sheet)
    })();
    let changed = match changed {
        Ok(sheet) => sheet,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if changed == old {
        // Nothing changed: no record is written and no step is made.
        return Ok(Ok(Applied::default()));
    }
    let mut sheets = world.sheets.clone();
    if let Err(e) = sheets.replace_sheet(changed.clone()) {
        return Ok(Err(e.to_string()));
    }
    run.write_all(vec![Write::Put(
        Box::new(changed.over(record)),
        Some(hash.clone()),
    )])
    .await
}

async fn delete_sheet<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, _, sheet) = match sheet_of(world, args.sheet.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut writes = Vec::new();
    for viewport in world.sheets.viewports_of(sheet.id()) {
        if let Some((viewport_hash, _)) = world.record(viewport.id()) {
            writes.push(Write::Remove(viewport.id().clone(), viewport_hash.clone()));
        }
    }
    // The schedules placed on the sheet go with it.
    let found: Vec<&Element> = world.all.values().map(|(_, record)| record).collect();
    if let Ok(placed) = construct_schedules::Placements::from_records(found) {
        for placement in placed.of_sheet(sheet.id()) {
            if let Some((placed_hash, _)) = world.record(placement.id()) {
                writes.push(Write::Remove(placement.id().clone(), placed_hash.clone()));
            }
        }
    }
    writes.push(Write::Remove(sheet.id().clone(), hash.clone()));
    run.write_all(writes).await
}

/// The viewport a command is about.
fn viewport_of(world: &World, id: Option<Id>) -> Result<(Id, Viewport), String> {
    let id = need(id, "a viewport")?;
    let missing = || construct_sheets::SheetError::UnknownViewport(id.clone()).to_string();
    let (hash, record) = world.record(&id).ok_or_else(missing)?;
    if record.kind != VIEWPORT_KIND {
        return Err(missing());
    }
    let viewport = Viewport::from_record(record).map_err(|e| e.to_string())?;
    Ok((hash.clone(), viewport))
}

async fn create_viewport<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let made = (|| {
        let id = need(args.viewport.clone(), "a viewport")?;
        let sheet_id = need(args.sheet.clone(), "a sheet")?;
        let view = need(args.view.clone(), "a view")?;
        let sheet = world.sheets.sheet(&sheet_id).ok_or_else(|| {
            construct_sheets::SheetError::UnknownSheet(sheet_id.clone()).to_string()
        })?;
        if world.views.get(&view).is_none() {
            return Err(construct_sheets::SheetError::UnknownView(view).to_string());
        }
        let half = |size: Length| Length::from_steps(size.steps().div_euclid(2));
        let x = match &args.x {
            Some(text) => length(text)?,
            None => half(sheet.paper().width()),
        };
        let y = match &args.y {
            Some(text) => length(text)?,
            None => half(sheet.paper().height()),
        };
        let viewport =
            Viewport::new(id, sheet_id, view, x, y).with_title(args.title.unwrap_or(true));
        Ok::<_, String>(viewport)
    })();
    let viewport = match made {
        Ok(viewport) => viewport,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if world.record(viewport.id()).is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let mut sheets = world.sheets.clone();
    if let Err(e) = sheets.add_viewport(viewport.clone()) {
        return Ok(Err(e.to_string()));
    }
    run.write_all(vec![Write::Put(
        Box::new(viewport.to_record(run.document)),
        None,
    )])
    .await
}

async fn put_viewport<T: Txn>(
    run: &mut Run<'_, T>,
    world: &World,
    before: (Id, Viewport),
    changed: Viewport,
) -> Answer {
    let (hash, old) = before;
    if changed == old {
        return Ok(Ok(Applied::default()));
    }
    let mut sheets = world.sheets.clone();
    if let Err(e) = sheets.replace_viewport(changed.clone()) {
        return Ok(Err(e.to_string()));
    }
    run.write_all(vec![Write::Put(
        Box::new(changed.to_record(run.document)),
        Some(hash),
    )])
    .await
}

async fn set_viewport<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, old) = match viewport_of(world, args.viewport.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if args.sheet.is_some() || args.view.is_some() {
        return Ok(Err(
            "A viewport keeps its sheet and its view: take it away and place the view again."
                .to_owned(),
        ));
    }
    let changed = (|| {
        let x = match &args.x {
            Some(text) => length(text)?,
            None => old.x(),
        };
        let y = match &args.y {
            Some(text) => length(text)?,
            None => old.y(),
        };
        Ok::<_, String>(
            old.clone()
                .at(x, y)
                .with_title(args.title.unwrap_or_else(|| old.title())),
        )
    })();
    match changed {
        Ok(changed) => put_viewport(run, world, (hash, old), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

fn alignment_over(change: &AlignChange) -> Result<Option<Alignment>, String> {
    match change {
        AlignChange::Away => Ok(None),
        AlignChange::To(args) => alignment_of(args).map(Some),
    }
}

fn alignment_of(args: &AlignArgs) -> Result<Alignment, String> {
    let anchor = Anchor::from_key(&args.anchor)
        .ok_or_else(|| format!("{:?} is not an anchor: min, centre or max.", args.anchor))?;
    Ok(Alignment {
        to: args.to.clone(),
        anchor,
    })
}

async fn align_viewport<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, old) = match viewport_of(world, args.viewport.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if args.across.is_none() && args.up.is_none() {
        return Ok(Err(
            "This command needs an alignment: across, up, or both (null takes one away)."
                .to_owned(),
        ));
    }
    let changed = (|| {
        let mut viewport = old.clone();
        if let Some(across) = &args.across {
            viewport = viewport.with_align_x(alignment_over(across)?);
        }
        if let Some(up) = &args.up {
            viewport = viewport.with_align_y(alignment_over(up)?);
        }
        Ok::<_, String>(viewport)
    })();
    match changed {
        Ok(changed) => put_viewport(run, world, (hash, old), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

async fn delete_viewport<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, viewport) = match viewport_of(world, args.viewport.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut sheets = world.sheets.clone();
    if let Err(e) = sheets.remove_viewport(viewport.id()) {
        return Ok(Err(e.to_string()));
    }
    run.write_all(vec![Write::Remove(viewport.id().clone(), hash)])
        .await
}

/// Why a view cannot be deleted: it is placed on a sheet. `None` when it is not (or no sheet
/// record reads, which the sheet commands report).
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn placed_refusal<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    view: &Id,
) -> Result<Option<String>, StoreError> {
    let Ok(world) = read_world(reader, project, document).await? else {
        return Ok(None);
    };
    Ok(world.sheets.placement_of(view).map(|(sheet, viewport)| {
        format!(
            "The view is placed on sheet {} ({}); take it off with viewport.delete {} first.",
            sheet.number(),
            sheet.name(),
            viewport.id()
        )
    }))
}

/// The part of a generic command the rules below read.
#[derive(Deserialize, Default)]
struct GenericArgs {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    kind: Option<String>,
    recipe: Option<construct_model::geometry::Recipe>,
}

/// Whether a recipe is a sheet's or a viewport's.
fn a_sheet_recipe(recipe: &construct_model::geometry::Recipe) -> bool {
    records::category_of(recipe).is_ok_and(|category| category == SHEETS || category == VIEWPORTS)
}

/// The generic commands do not go behind the sheet commands: `element.create` of a sheet or a
/// viewport and `element.set`, `element.move` or `element.delete` of one are refused, naming the
/// command to use. Anything else is left to the command.
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
    let by_recipe = args.recipe.as_ref().is_some_and(a_sheet_recipe);
    let by_kind = matches!(args.kind.as_deref(), Some(SHEET_KIND | VIEWPORT_KIND));
    let use_instead = |what: &str, command: &str| format!("{what}: use {command}.");
    Ok(match edit.cmd.as_str() {
        "element.create" if by_recipe || by_kind => Err(use_instead(
            "A sheet or a viewport is made",
            "sheet.create or viewport.create",
        )),
        "element.set" | "element.move" | "element.delete" => {
            let Some(id) = &args.element else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, &edit.target, id).await? {
                Some((_, record)) if record.kind == SHEET_KIND => Err(match edit.cmd.as_str() {
                    "element.delete" => use_instead("A sheet is deleted", "sheet.delete"),
                    _ => use_instead(
                        "A sheet is changed",
                        "sheet.set (parameters: element.param)",
                    ),
                }),
                Some((_, record)) if record.kind == VIEWPORT_KIND => Err(match edit.cmd.as_str() {
                    "element.delete" => use_instead("A viewport is deleted", "viewport.delete"),
                    _ => use_instead("A viewport is changed", "viewport.set or viewport.align"),
                }),
                Some(_) if by_recipe => Err(use_instead(
                    "A sheet or a viewport is made",
                    "sheet.create or viewport.create",
                )),
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    })
}

/// Whether the sheets an undo wrote still stand: every record reads, every number is free, every
/// viewport is on a sheet and shows a view that is there, no view is placed twice, every viewport
/// is on its paper and every alignment holds. The whole document is checked whenever a sheet, a
/// viewport or a view was written or taken away.
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
        .any(|record| matches!(record.kind.as_str(), SHEET_KIND | VIEWPORT_KIND | "view"));
    // A piece taken away may be a view, a sheet or a viewport: its kind is not known any more.
    if !touched && deleted.is_empty() {
        return Ok(Ok(()));
    }
    let world = match read_world(txn, project, document).await? {
        Ok(world) => world,
        Err(why) => return Ok(Err(why)),
    };
    // Build the document again through the rules, in the order they need.
    let mut rebuilt = Sheets::new();
    for sheet in world.sheets.ordered() {
        if let Err(e) = rebuilt.add_sheet(sheet.clone()) {
            return Ok(Err(e.to_string()));
        }
    }
    let mut viewports = world.sheets.viewports();
    // Followed viewports are added before those that follow them, so every alignment can be
    // checked as it is added.
    viewports.sort_by_key(|viewport| {
        (
            [viewport.align_x(), viewport.align_y()]
                .into_iter()
                .flatten()
                .count(),
            viewport.id().clone(),
        )
    });
    let mut pending: Vec<Viewport> = viewports.into_iter().cloned().collect();
    while !pending.is_empty() {
        let before = pending.len();
        let mut later = Vec::new();
        for viewport in pending {
            if world.views.get(viewport.view()).is_none() {
                return Ok(Err(format!(
                    "{}",
                    construct_sheets::SheetError::UnknownView(viewport.view().clone())
                )));
            }
            let ready = [viewport.align_x(), viewport.align_y()]
                .into_iter()
                .flatten()
                .all(|align| rebuilt.viewport(&align.to).is_some());
            if ready {
                if let Err(e) = rebuilt.add_viewport(viewport) {
                    return Ok(Err(e.to_string()));
                }
            } else {
                later.push(viewport);
            }
        }
        if later.len() == before {
            let first = later
                .first()
                .map(|v| v.id().to_string())
                .unwrap_or_default();
            return Ok(Err(format!(
                "The viewport {first} is aligned to a viewport that is not there, or in a loop."
            )));
        }
        pending = later;
    }
    Ok(Ok(()))
}
