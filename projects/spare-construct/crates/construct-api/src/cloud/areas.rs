//! The commands that make and change area schemes, area boundary lines and areas (job 14.6,
//! CON-A4-F7-P2). They go through the same edit log as `element.*` and the room commands
//! ([`crate::cloud::rooms`], which routes them), with the same access check (`model.edit`), the same
//! claims and one entry each, so one undo takes back all one wrote. An area scheme (gross, rentable,
//! or another measure) holds areas; an area plan view ([`crate::cloud::views`], kind `area_plan`)
//! shows one scheme on one level; an area boundary line bounds the areas of its scheme and level
//! and nothing else, and is its own record, never a room line; an area is found by a point inside
//! boundary lines, as a room is inside walls, and its loop, area and perimeter are worked out from
//! them, never stored (`crates/construct-rooms/FORMAT.md`).
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `areascheme.add` | `element`, `name`, `measure` (`gross`, `rentable` or `other`, default `gross`) | A new area scheme |
//! | `areascheme.rename` | `element`, `name` | The scheme: its name |
//! | `areascheme.delete` | `element` | The scheme goes; refused while an area, a boundary line or an area plan uses it |
//! | `arealine.draw` | `element`, `scheme`, `level`, `offset` (default `0`), `from` and `to` (`[x, y]` typed text) | A new area boundary line |
//! | `arealine.pick` | `element`, `scheme`, `wall`, `near` (`[x, y]` typed text) | A new area boundary line along the face of the wall `wall` nearest the point `near`, on the wall's level; the line is its own record and does not follow the wall |
//! | `arealine.move` | `element`, `from`, `to` | The line: its two ends |
//! | `arealine.delete` | `element` | The line goes |
//! | `area.place` | `element`, `scheme`, `name` (default `Area`), `number` (default the next free one in the scheme), `level`, `offset`, `at` (`[x, y]`) | A new area |
//! | `area.rename` | `element`, `name` | The area: its name |
//! | `area.number` | `element`, `number` | The area: its number |
//! | `area.move` | `element`, `at` | The area: its point |
//! | `area.delete` | `element` | The area goes |
//!
//! "Not enclosed" and "redundant" are not refusals for an area, as for a room: they are what became
//! of it and its record stays. Every refusal of the search ([`construct_regen::rooms::area_worked_at`])
//! is the command's, in the search's own words. An area number is unique in its scheme. The generic
//! commands do not go behind these ([`generic_rules`]).
//!
//! For the page, without the log: [`read_areas`], the schemes, and the next free number in each.

use crate::cloud::levels;
use crate::cloud::rooms::{Answer, Args, Run, ends, length, need, point, why};
use construct_levels::Tie;
use construct_model::element::Element;
use construct_params::Length;
use construct_regen::parameters::{self as records, At};
use construct_rooms::{
    LineKind, Measure, SCHEME_KEY, area_line_record, area_number_of, area_recipe, area_record,
    is_area, is_area_line, is_scheme, line_recipe, measure_of, nearest_edge, next_number,
    scheme_of, scheme_record,
};
use kit_core::Id;
use kit_ops::Edit;
use kit_store::{Reader, StoreError, Txn};
use serde::{Deserialize, Serialize};

/// Every command here.
pub const COMMANDS: [&str; 12] = [
    "areascheme.add",
    "areascheme.rename",
    "areascheme.delete",
    "arealine.draw",
    "arealine.pick",
    "arealine.move",
    "arealine.delete",
    "area.place",
    "area.rename",
    "area.number",
    "area.move",
    "area.delete",
];

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// The name an area is given when none is.
const DEFAULT_NAME: &str = "Area";

/// A name that is not blank.
fn named(name: Option<String>, default: Option<&str>, what: &str) -> Result<String, String> {
    match name {
        Some(name) if name.trim().is_empty() => Err(format!("{what} needs a name.")),
        Some(name) => Ok(name.trim().to_owned()),
        None => default
            .map(str::to_owned)
            .ok_or_else(|| "This command needs a name.".to_owned()),
    }
}

impl<T: Txn> Run<'_, T> {
    /// Every record of the document.
    async fn records(&mut self) -> Result<Vec<Element>, StoreError> {
        let all = records::read_document(self.txn, self.project, self.document).await?;
        Ok(all.into_values().map(|(_, record)| record).collect())
    }

    /// The area scheme `scheme`, or the refusal.
    async fn scheme(&mut self, scheme: Option<Id>) -> Result<Result<Element, String>, StoreError> {
        let id = match need(scheme, "an area scheme, `scheme`") {
            Ok(id) => id,
            Err(refusal) => return Ok(Err(refusal)),
        };
        Ok(match self.current(&id).await? {
            Some((_, record)) if is_scheme(&record) => Ok(record),
            _ => Err(format!("{id} is not an area scheme of this model.")),
        })
    }

    /// The element a command is about, of the kind `is`, or the refusal naming `what`.
    async fn target_of(
        &mut self,
        element: Option<Id>,
        is: fn(&Element) -> bool,
        what: &str,
    ) -> Result<Result<(Id, Element), String>, StoreError> {
        let id = match need(element, "an element") {
            Ok(id) => id,
            Err(refusal) => return Ok(Err(refusal)),
        };
        let Some((hash, record)) = self.current(&id).await? else {
            return Ok(Err("That element no longer exists.".to_owned()));
        };
        if !is(&record) {
            return Ok(Err(format!("{id} is not {what}.")));
        }
        Ok(Ok((hash, record)))
    }

    /// What the builders would make of the area `record`: the refusal, in the search's own words,
    /// when they would refuse it. Not enclosed, or in a loop an earlier area already has, is not
    /// refused: it is what became of it.
    async fn area_buildable(&mut self, record: &Element) -> Result<Result<(), String>, StoreError> {
        let worked = construct_regen::rooms::area_worked_at(
            self.txn,
            self.project,
            self.document,
            record,
            At::Now,
        )
        .await?;
        Ok(match worked {
            Err(refusal) => Err(refusal.message),
            Ok((Ok(_), _)) => Ok(()),
            Ok((Err(failed), _)) => {
                if matches!(failed.code.as_str(), "NotEnclosed" | "Redundant") {
                    Ok(())
                } else {
                    Err(failed.message)
                }
            }
        })
    }

    /// The refusal when `number` is already another area's in the scheme; the area is named.
    async fn area_number_free(
        &mut self,
        number: &str,
        scheme: &Id,
        area: &Id,
    ) -> Result<Result<(), String>, StoreError> {
        let taken = self.records().await?.into_iter().find(|other| {
            is_area(other)
                && other.id != *area
                && scheme_of(other).as_ref() == Some(scheme)
                && area_number_of(other) == Some(number)
        });
        Ok(match taken {
            Some(other) => Err(format!(
                "Number {number} is already used by area {} in this scheme: give this area another number.",
                other.name
            )),
            None => Ok(()),
        })
    }
}

/// Runs one of these commands, given the run the room commands set up.
///
/// # Errors
///
/// A store error; a command that cannot be made is the outcome's error instead.
pub(crate) async fn apply_run<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    match run.edit.cmd.as_str() {
        "areascheme.add" => scheme_add(run, args).await,
        "areascheme.rename" => scheme_rename(run, args).await,
        "areascheme.delete" => scheme_delete(run, args).await,
        "arealine.draw" => line_draw(run, args).await,
        "arealine.pick" => line_pick(run, args).await,
        "arealine.move" => line_move(run, args).await,
        "arealine.delete" => line_delete(run, args).await,
        "area.place" => place(run, args).await,
        "area.rename" => rename(run, args).await,
        "area.number" => renumber(run, args).await,
        "area.move" => move_to(run, args).await,
        "area.delete" => delete(run, args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

async fn scheme_add<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let id = match need(args.element.clone(), "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(&id).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let name = match named(args.name.clone(), None, "An area scheme") {
        Ok(name) => name,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let measure = match args.measure.as_deref() {
        None => Measure::Gross,
        Some(word) => match Measure::from_word(word) {
            Some(measure) => measure,
            None => {
                return Ok(Err(format!(
                    "{word:?} is not a measure. The measures are gross, rentable and other."
                )));
            }
        },
    };
    if let Some(taken) = scheme_named(run, &name, &id).await? {
        return Ok(Err(taken));
    }
    let record = match scheme_record(&id, run.document, &name, measure) {
        Ok(record) => record,
        Err(refusal) => return Ok(Err(why(refusal))),
    };
    run.put_claimed(&record).await
}

/// The refusal when another scheme already has `name` (case aside).
async fn scheme_named<T: Txn>(
    run: &mut Run<'_, T>,
    name: &str,
    scheme: &Id,
) -> Result<Option<String>, StoreError> {
    let wanted = name.to_lowercase();
    Ok(run
        .records()
        .await?
        .into_iter()
        .find(|other| {
            is_scheme(other) && other.id != *scheme && other.name.to_lowercase() == wanted
        })
        .map(|_| {
            format!("There is already an area scheme named {name}: give this one another name.")
        }))
}

async fn scheme_rename<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, mut record) = match run
        .target_of(args.element.clone(), is_scheme, "an area scheme")
        .await?
    {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let name = match named(args.name, None, "An area scheme") {
        Ok(name) => name,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Some(taken) = scheme_named(run, &name, &record.id).await? {
        return Ok(Err(taken));
    }
    record.name = name;
    run.put_claimed(&record).await
}

async fn scheme_delete<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (hash, record) = match run
        .target_of(args.element, is_scheme, "an area scheme")
        .await?
    {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let users = run
        .records()
        .await?
        .into_iter()
        .filter(|other| other.props.get(SCHEME_KEY).map(String::as_str) == Some(record.id.as_str()))
        .count();
    if users > 0 {
        return Ok(Err(format!(
            "{} is used by {users} area{}, boundary line{} or area plan{}: delete those first.",
            record.name,
            if users == 1 { "" } else { "s" },
            if users == 1 { "" } else { "s" },
            if users == 1 { "" } else { "s" },
        )));
    }
    run.take_away(&record.id, hash).await
}

/// The base tie a command draws on: its `level` and `offset`.
async fn base<T: Txn>(
    run: &mut Run<'_, T>,
    args: &Args,
    what: &str,
) -> Result<Result<Tie, String>, StoreError> {
    let base =
        match levels::tie_of_new(run.txn, run.project, run.document, args.level.as_ref()).await? {
            Ok(Some(base)) => base,
            Ok(None) => {
                return Ok(Err(format!(
                    "{what} stands on a level: this command needs a level."
                )));
            }
            Err(refusal) => return Ok(Err(refusal)),
        };
    Ok(match args.offset.as_deref().map(length) {
        None => Ok(base),
        Some(Ok(offset)) => Ok(Tie { offset, ..base }),
        Some(Err(refusal)) => Err(refusal),
    })
}

async fn line_draw<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let id = match need(args.element.clone(), "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(&id).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let scheme = match run.scheme(args.scheme.clone()).await? {
        Ok(scheme) => scheme,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let (from, to) = match ends(args.from.clone(), args.to.clone()) {
        Ok(ends) => ends,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let base = match base(run, &args, "An area boundary line").await? {
        Ok(base) => base,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let record = match area_line_record(&id, run.document, &scheme.id, &base, from, to) {
        Ok(record) => record,
        Err(refusal) => return Ok(Err(why(refusal))),
    };
    run.put_claimed(&record).await
}

async fn line_pick<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let id = match need(args.element.clone(), "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(&id).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let scheme = match run.scheme(args.scheme.clone()).await? {
        Ok(scheme) => scheme,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let wall = match need(args.wall.clone(), "a wall, `wall`") {
        Ok(wall) => wall,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Ok(near) = point(args.near.clone()) else {
        return Ok(Err(
            "This command needs the point clicked, `near`, to say which face.".to_owned(),
        ));
    };
    let Some((_, wall_record)) = run.current(&wall).await? else {
        return Ok(Err("That wall no longer exists.".to_owned()));
    };
    if wall_record.kind != construct_regen::rooms::WALL_KIND {
        return Ok(Err(format!("{} is not a wall.", wall_record.name)));
    }
    let Some(level) = construct_regen::rooms::level_of(&wall_record) else {
        return Ok(Err(format!("{} stands on no level.", wall_record.name)));
    };
    let bounds =
        construct_regen::rooms::bounds_on(run.txn, run.project, run.document, &level, At::Now)
            .await?;
    let outline = bounds.into_iter().find_map(|bound| match bound.outline {
        construct_rooms::Outline::Polygon(corners) if bound.wall == wall => Some(corners),
        _ => None,
    });
    let Some(outline) = outline else {
        return Ok(Err(format!(
            "{} has no outline in plan to pick a face from.",
            wall_record.name
        )));
    };
    let Some([from, to]) = nearest_edge(&outline, [near[0].steps(), near[1].steps()]) else {
        return Ok(Err(format!("{} has no face to pick.", wall_record.name)));
    };
    let at = |p: [i64; 2]| [Length::from_steps(p[0]), Length::from_steps(p[1])];
    let tie = Tie {
        level,
        offset: Length::from_steps(0),
    };
    let record = match area_line_record(&id, run.document, &scheme.id, &tie, at(from), at(to)) {
        Ok(record) => record,
        Err(refusal) => return Ok(Err(why(refusal))),
    };
    run.put_claimed(&record).await
}

async fn line_move<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, mut record) = match run
        .target_of(args.element.clone(), is_area_line, "an area boundary line")
        .await?
    {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let (from, to) = match ends(args.from, args.to) {
        Ok(ends) => ends,
        Err(refusal) => return Ok(Err(refusal)),
    };
    record.recipe = match line_recipe(LineKind::Area, from, to) {
        Ok(recipe) => recipe,
        Err(refusal) => return Ok(Err(why(refusal))),
    };
    run.put_claimed(&record).await
}

async fn line_delete<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (hash, record) = match run
        .target_of(args.element, is_area_line, "an area boundary line")
        .await?
    {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    run.take_away(&record.id, hash).await
}

async fn place<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let id = match need(args.element.clone(), "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(&id).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let scheme = match run.scheme(args.scheme.clone()).await? {
        Ok(scheme) => scheme,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let name = match named(args.name.clone(), Some(DEFAULT_NAME), "An area") {
        Ok(name) => name,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let at = match point(args.at.clone()) {
        Ok(at) => at,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let base = match base(run, &args, "An area").await? {
        Ok(base) => base,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let number = match args.number.as_deref().map(str::trim) {
        Some("") => return Ok(Err("An area needs a number.".to_owned())),
        Some(number) => {
            if let Err(refusal) = run.area_number_free(number, &scheme.id, &id).await? {
                return Ok(Err(refusal));
            }
            number.to_owned()
        }
        None => next_in(run, &scheme.id).await?,
    };
    let record = area_record(&id, run.document, &scheme.id, &name, &number, at, &base);
    if let Err(refusal) = run.area_buildable(&record).await? {
        return Ok(Err(refusal));
    }
    run.put_claimed(&record).await
}

/// The next free area number in the scheme.
async fn next_in<T: Txn>(run: &mut Run<'_, T>, scheme: &Id) -> Result<String, StoreError> {
    let areas: Vec<Element> = run
        .records()
        .await?
        .into_iter()
        .filter(|other| is_area(other) && scheme_of(other).as_ref() == Some(scheme))
        .collect();
    Ok(next_number(areas.iter().filter_map(area_number_of)))
}

async fn rename<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, mut record) = match run
        .target_of(args.element.clone(), is_area, "an area")
        .await?
    {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    record.name = match named(args.name, None, "An area") {
        Ok(name) => name,
        Err(refusal) => return Ok(Err(refusal)),
    };
    run.put_claimed(&record).await
}

async fn renumber<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, mut record) = match run
        .target_of(args.element.clone(), is_area, "an area")
        .await?
    {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let number = match need(args.number, "a number") {
        Ok(number) => number.trim().to_owned(),
        Err(refusal) => return Ok(Err(refusal)),
    };
    if number.is_empty() {
        return Ok(Err("An area needs a number.".to_owned()));
    }
    let Some(scheme) = scheme_of(&record) else {
        return Ok(Err(format!("{} has no area scheme.", record.name)));
    };
    if let Err(refusal) = run.area_number_free(&number, &scheme, &record.id).await? {
        return Ok(Err(refusal));
    }
    record
        .props
        .insert(construct_rooms::AREA_NUMBER_KEY.to_owned(), number);
    run.put_claimed(&record).await
}

async fn move_to<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, mut record) = match run
        .target_of(args.element.clone(), is_area, "an area")
        .await?
    {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let at = match point(args.at) {
        Ok(at) => at,
        Err(refusal) => return Ok(Err(refusal)),
    };
    record.recipe = area_recipe(at);
    if let Err(refusal) = run.area_buildable(&record).await? {
        return Ok(Err(refusal));
    }
    run.put_claimed(&record).await
}

async fn delete<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (hash, record) = match run.target_of(args.element, is_area, "an area").await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    run.take_away(&record.id, hash).await
}

/// The part of a generic command's arguments these rules read.
#[derive(Deserialize, Default)]
struct GenericArgs {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    kind: Option<String>,
    recipe: Option<construct_model::geometry::Recipe>,
}

/// The command to use for a kind of record these commands own, or none.
fn command_for(kind: &str, cmd: &str) -> Option<(&'static str, &'static str)> {
    let (noun, make, change) = match kind {
        construct_rooms::ROOM_LINE_KIND => (
            "A room separation line",
            "roomline.draw",
            "roomline.move or roomline.delete",
        ),
        construct_rooms::AREA_LINE_KIND => (
            "An area boundary line",
            "arealine.draw, arealine.pick",
            "arealine.move or arealine.delete",
        ),
        construct_rooms::AREA_KIND => (
            "An area",
            "area.place",
            "area.rename, area.number, area.move or area.delete",
        ),
        construct_rooms::SCHEME_KIND => (
            "An area scheme",
            "areascheme.add",
            "areascheme.rename or areascheme.delete",
        ),
        _ => return None,
    };
    Some(if cmd == "element.create" {
        (noun, make)
    } else {
        (noun, change)
    })
}

/// The generic commands' rules for what these commands own, asked before one runs: a room
/// separation line, an area boundary line, an area and an area scheme are made and changed by their
/// own commands, not `element.create`, `element.set`, `element.move` or `element.delete`. The inner
/// error is the refusal, naming the command to use. An edit these rules have nothing to say of is
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
    let cmd = edit.cmd.as_str();
    if cmd == "element.create" {
        let kind = args
            .kind
            .clone()
            .or_else(|| args.recipe.as_ref().and_then(kind_of_recipe));
        if let Some(kind) = kind
            && let Some((noun, make)) = command_for(&kind, cmd)
        {
            return Ok(Err(format!("{noun} is made with {make}, which checks it.")));
        }
        return Ok(Ok(()));
    }
    if !matches!(cmd, "element.set" | "element.move" | "element.delete") {
        return Ok(Ok(()));
    }
    let Some(id) = &args.element else {
        return Ok(Ok(()));
    };
    Ok(
        match records::read_element(txn, project, &edit.target, id).await? {
            Some((_, record)) => match command_for(&record.kind, cmd) {
                Some((noun, change)) => {
                    let who = if record.name.is_empty() {
                        record.id.to_string()
                    } else {
                        record.name.clone()
                    };
                    Err(format!(
                        "{noun} {who} is changed with {change}, which checks it."
                    ))
                }
                None => Ok(()),
            },
            None => Ok(()),
        },
    )
}

/// The element kind a recipe of ours makes, if it is one of these commands'.
fn kind_of_recipe(recipe: &construct_model::geometry::Recipe) -> Option<String> {
    if recipe.owner != "construct" {
        return None;
    }
    match recipe.op.as_str() {
        construct_rooms::ROOM_LINE_OP => Some(construct_rooms::ROOM_LINE_KIND.to_owned()),
        construct_rooms::AREA_LINE_OP => Some(construct_rooms::AREA_LINE_KIND.to_owned()),
        construct_rooms::AREA_OP => Some(construct_rooms::AREA_KIND.to_owned()),
        construct_rooms::SCHEME_OP => Some(construct_rooms::SCHEME_KIND.to_owned()),
        _ => None,
    }
}

/// What the page asks of a document's area schemes.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct AreasView {
    /// Every area scheme, by name (case aside), then by ID.
    pub schemes: Vec<SchemeView>,
}

/// One area scheme as the page needs it.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct SchemeView {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// What it measures: `gross`, `rentable` or `other`.
    pub measure: String,
    /// The next free area number in it: what a person placing an area is offered.
    pub next_number: String,
}

/// A document's area schemes as the page needs them, each with the next free area number.
///
/// # Errors
/// A store error.
pub async fn read_areas<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<AreasView, StoreError> {
    let all: Vec<Element> = records::read_document(reader, project, document)
        .await?
        .into_values()
        .map(|(_, record)| record)
        .collect();
    let mut schemes: Vec<SchemeView> = all
        .iter()
        .filter(|record| is_scheme(record))
        .map(|scheme| SchemeView {
            id: scheme.id.to_string(),
            name: scheme.name.clone(),
            measure: measure_of(scheme).map_or("other", Measure::word).to_owned(),
            next_number: next_number(
                all.iter()
                    .filter(|other| is_area(other) && scheme_of(other).as_ref() == Some(&scheme.id))
                    .filter_map(area_number_of),
            ),
        })
        .collect();
    schemes.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(AreasView { schemes })
}
