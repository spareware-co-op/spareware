//! The commands that place and change rooms (job 5.4, CON-A4-F7-P1, second half). They go through
//! the same edit log as `element.*`, with the same access check (`model.edit`) and the same
//! claims: a room another person is working on is refused, and one person's edits land in log
//! order. Each is one entry, so one undo takes back all it wrote ([`crate::cloud::undo`]). They
//! read as the floor commands do ([`crate::cloud::floors`]).
//!
//! A room is a point on a level with a name, a number and an upper limit; its loop, boundary and
//! measures are found from the walls round it by construct-rooms and regeneration, never stored
//! (`crates/construct-rooms/FORMAT.md`). So a room whose walls change is built again by
//! regeneration and has no command of its own for it. Each command checks the room with
//! construct-rooms' own rules and with the very search the builders use
//! ([`construct_regen::rooms::found_at`]), so a room a command takes is one the builders take.
//! "Not enclosed" and "redundant" are not refusals: they are what became of the room, and its
//! record stays; every refusal of the search is the command's, in the search's own words.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `room.place` | `element`, `name` (default `Room`), `number` (default the next free one), `level`, `offset` (default `0`), `at` (`[x, y]` typed text), then `top` and `top_offset` or `height` | A new room |
//! | `room.rename` | `element`, `name` | The room: its name |
//! | `room.number` | `element`, `number` | The room: its number |
//! | `room.move` | `element`, `at` | The room: its point |
//! | `room.upper` | `element`, then `top` and `top_offset` or `height` | The room: its upper limit |
//! | `room.delete` | `element` | The room goes |
//!
//! `at` is the room's point in the model's plan, two lengths as typed text. The upper limit is the
//! level `top` (its ID), `top_offset` above it, or `height` (typed text) above the room's base:
//! one or the other. A room's base level and offset are changed by `level.tie`, as any element's.
//!
//! Refused, in words: an element of that ID; no level or one not in the model; a point in a wall,
//! a curved wall on the level, a corner off the length grid or an area that is no whole number of
//! square grid steps (construct-rooms' words); an upper limit at or below the base, none given or
//! both; a blank name; a number that is blank or already used by another room (the room is named);
//! an element that is not a room; text that is no length.
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of a room,
//! `element.set` or `element.move` of one and `element.delete` of one are refused, naming the
//! command to use.
//!
//! For the page, without the log: [`read_rooms`], the next free number it offers a person placing
//! a room.
//!
//! Typed text is read with construct-params' metric display settings until a project keeps its
//! own. Later, with their places: room separation lines and rooms bounded by floors and ceilings
//! (job 6.3), and a sloped upper limit (the interim register).

use crate::cloud::elements::ElementTool;
use crate::cloud::elements::write_record;
use crate::cloud::levels;
use crate::cloud::tool::Tool;
use construct_levels::{Length, Tie, parse_length, write_top};
use construct_model::element::Element;
use construct_params::Units;
use construct_regen::parameters::{self as records, At};
use construct_rooms::{
    Limit, NUMBER_KEY, RoomError, at_of, is_room, is_room_recipe, limit_of, next_number, number_of,
    room_recipe, room_record,
};
use kit_core::Id;
use kit_ops::{Change, Claim, ClaimChange, Edit};
use kit_store::{Place, Reader, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::{Deserialize, Serialize};

/// Every command here.
pub const COMMANDS: [&str; 6] = [
    "room.place",
    "room.rename",
    "room.number",
    "room.move",
    "room.upper",
    "room.delete",
];

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// The room tool ([`Tool`]): placing and changing rooms.
pub struct RoomTool;

impl Tool for RoomTool {
    const NAME: &'static str = "room";
    const COMMANDS: &'static [&'static str] = &COMMANDS;
    const GUARDS: &'static [&'static str] = &[ElementTool::NAME];

    async fn apply<T: Txn>(
        txn: &mut T,
        project: &Id,
        context: &kit_sync::Context<'_>,
        edit: &Edit,
        claim_ms: u64,
    ) -> Result<Outcome, StoreError> {
        apply(txn, project, context, edit, claim_ms).await
    }

    async fn guard<T: Txn>(
        txn: &mut T,
        project: &Id,
        edit: &Edit,
    ) -> Result<Result<(), String>, StoreError> {
        generic_rules(txn, project, edit).await
    }
}

/// The name a room is given when none is.
const DEFAULT_NAME: &str = "Room";

/// The arguments of every command here; each uses some of them.
#[derive(Deserialize, Default)]
struct Args {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    name: Option<String>,
    number: Option<String>,
    /// The base level: its ID, or an old page's integer ([`levels::tie_of_new`]).
    level: Option<serde_json::Value>,
    offset: Option<String>,
    at: Option<[String; 2]>,
    #[serde(default, with = "kit_ops::id::option")]
    top: Option<Id>,
    top_offset: Option<String>,
    height: Option<String>,
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

/// Turns a data crate's refusal into the command's result.
fn why(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// How typed lengths are read: construct-params' metric display settings, until a project keeps
/// its own.
fn units() -> Units {
    Units::default()
}

/// A typed length, or construct-levels' refusal of the text.
fn length(text: &str) -> Result<Length, String> {
    parse_length(text, &units()).map_err(why)
}

/// A room's point from what a person sent.
fn point(sent: Option<[String; 2]>) -> Result<[Length; 2], String> {
    let [x, y] = need(sent, "a point, `at`")?;
    Ok([length(&x)?, length(&y)?])
}

/// A name that is not blank.
fn named(name: Option<String>, default: Option<&str>) -> Result<String, String> {
    match name {
        Some(name) if name.trim().is_empty() => Err("A room needs a name.".to_owned()),
        Some(name) => Ok(name.trim().to_owned()),
        None => default
            .map(str::to_owned)
            .ok_or_else(|| "This command needs a name.".to_owned()),
    }
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

    /// Writes a record and points its piece at it; the change, from what it was.
    async fn put(&mut self, record: &Element) -> Result<Change, StoreError> {
        let before = self.current(&record.id).await?.map(|(hash, _)| hash);
        let hash = write_record(self.txn, self.project, record).await?;
        let pointer = Place::Piece {
            project: self.project.clone(),
            document: self.document.clone(),
            piece: record.id.clone(),
        };
        self.txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
        Ok(Change {
            piece: record.id.clone(),
            before,
            after: Some(hash),
        })
    }

    /// Writes one record whose piece this person claims, as the command's whole result.
    async fn put_claimed(&mut self, record: &Element) -> Answer {
        let claim = match self.claim(&record.id).await? {
            Ok(claim) => claim,
            Err(refusal) => return Ok(Err(refusal)),
        };
        let change = self.put(record).await?;
        let claim = claims::take(self.txn, self.project, self.document, &record.id, claim).await?;
        Ok(Ok(Applied {
            changes: vec![change],
            claims: vec![claim],
            ..Applied::default()
        }))
    }

    /// The rooms of the document, by ID.
    async fn rooms(&mut self) -> Result<Vec<Element>, StoreError> {
        let all = records::read_document(self.txn, self.project, self.document).await?;
        Ok(all
            .into_values()
            .map(|(_, record)| record)
            .filter(is_room)
            .collect())
    }

    /// The refusal when `number` is already another room's; the room is named.
    async fn number_free(
        &mut self,
        number: &str,
        room: &Id,
    ) -> Result<Result<(), String>, StoreError> {
        let taken = self
            .rooms()
            .await?
            .into_iter()
            .find(|other| other.id != *room && number_of(other) == Some(number));
        Ok(match taken {
            Some(other) => Err(format!(
                "Number {number} is already used by room {}: give this room another number.",
                other.name
            )),
            None => Ok(()),
        })
    }

    /// What the builders would make of `record`, read the way they read it: the refusal, in the
    /// search's own words, when they would refuse it. A room not enclosed, or in a loop an earlier
    /// room already has, is not refused: it is what became of it.
    async fn buildable(&mut self, record: &Element) -> Result<Result<(), String>, StoreError> {
        let found = construct_regen::rooms::found_at(
            self.txn,
            self.project,
            self.document,
            record,
            At::Now,
        )
        .await?;
        Ok(found.map(|_| ()).map_err(|failed| failed.message))
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
        "room.place" => place(&mut run, args).await,
        "room.rename" => rename(&mut run, args).await,
        "room.number" => renumber(&mut run, args).await,
        "room.move" => move_to(&mut run, args).await,
        "room.upper" => upper(&mut run, args).await,
        "room.delete" => delete(&mut run, args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

/// The room a command is about: its record and hash, or the refusal.
async fn target<T: Txn>(
    run: &mut Run<'_, T>,
    element: Option<Id>,
) -> Result<Result<(Id, Element), String>, StoreError> {
    let id = match need(element, "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some((hash, record)) = run.current(&id).await? else {
        return Ok(Err("That element no longer exists.".to_owned()));
    };
    if !is_room(&record) {
        return Ok(Err(format!("{} is not a room.", record.name)));
    }
    Ok(Ok((hash, record)))
}

/// A room's upper limit from what a person sent: the level `top` names, `top_offset` above it, or
/// `height`. The inner error is the refusal, in words.
async fn limit<T: Txn>(
    run: &mut Run<'_, T>,
    args: &Args,
) -> Result<Result<Limit, String>, StoreError> {
    Ok(match (&args.top, &args.height, &args.top_offset) {
        (Some(top), None, offset) => {
            if let Err(refusal) = levels::level_now(run.txn, run.project, run.document, top).await?
            {
                return Ok(Err(refusal));
            }
            offset
                .as_deref()
                .map_or(Ok(Length::from_steps(0)), length)
                .map(|offset| {
                    Limit::Level(Tie {
                        level: top.clone(),
                        offset,
                    })
                })
        }
        (None, Some(_), Some(_)) => {
            Err("A top offset is measured from a top level, and none is given.".to_owned())
        }
        (None, Some(text), None) => length(text).map(Limit::Height),
        (Some(_), Some(_), _) => Err(
            "A room's upper limit is tied to a level or given as a height, not both.".to_owned(),
        ),
        (None, None, _) => {
            Err("A room needs its upper limit tied to a level or a height.".to_owned())
        }
    })
}

/// The record's upper limit ties written for `limit`: its top tie, or none for a given height.
fn write_limit(record: &mut Element, limit: &Limit) {
    match limit {
        Limit::Level(top) => write_top(&mut record.props, Some(top)),
        Limit::Height(_) | Limit::Sloped => write_top(&mut record.props, None),
    }
}

/// Refuses a room whose upper limit is at or below its base, by the document's levels as they are
/// now.
async fn above_base<T: Txn>(
    run: &mut Run<'_, T>,
    record: &Element,
) -> Result<Result<(), String>, StoreError> {
    levels::top_is_above_base(run.txn, run.project, run.document, record).await
}

async fn place<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let id = match need(args.element.clone(), "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(&id).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let name = match named(args.name.clone(), Some(DEFAULT_NAME)) {
        Ok(name) => name,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let at = match point(args.at.clone()) {
        Ok(at) => at,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let base =
        match levels::tie_of_new(run.txn, run.project, run.document, args.level.as_ref()).await? {
            Ok(Some(base)) => base,
            Ok(None) => {
                return Ok(Err(
                    "A room stands on a level: this command needs a level.".to_owned()
                ));
            }
            Err(refusal) => return Ok(Err(refusal)),
        };
    let base = match args.offset.as_deref().map(length) {
        None => base,
        Some(Ok(offset)) => Tie { offset, ..base },
        Some(Err(refusal)) => return Ok(Err(refusal)),
    };
    let limit = match limit(run, &args).await? {
        Ok(limit) => limit,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let number = match args.number.as_deref().map(str::trim) {
        Some("") => return Ok(Err("A room needs a number.".to_owned())),
        Some(number) => {
            if let Err(refusal) = run.number_free(number, &id).await? {
                return Ok(Err(refusal));
            }
            number.to_owned()
        }
        None => {
            let rooms = run.rooms().await?;
            next_number(rooms.iter().filter_map(number_of))
        }
    };
    let record = match room_record(&id, run.document, &name, &number, at, &base, &limit) {
        Ok(record) => record,
        Err(refusal) => return Ok(Err(why(refusal))),
    };
    if let Err(refusal) = above_base(run, &record).await? {
        return Ok(Err(refusal));
    }
    if let Err(refusal) = run.buildable(&record).await? {
        return Ok(Err(refusal));
    }
    run.put_claimed(&record).await
}

async fn rename<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, mut record) = match target(run, args.element.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    record.name = match named(args.name, None) {
        Ok(name) => name,
        Err(refusal) => return Ok(Err(refusal)),
    };
    run.put_claimed(&record).await
}

async fn renumber<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, mut record) = match target(run, args.element.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let number = match need(args.number, "a number") {
        Ok(number) => number.trim().to_owned(),
        Err(refusal) => return Ok(Err(refusal)),
    };
    if number.is_empty() {
        return Ok(Err("A room needs a number.".to_owned()));
    }
    if let Err(refusal) = run.number_free(&number, &record.id).await? {
        return Ok(Err(refusal));
    }
    record.props.insert(NUMBER_KEY.to_owned(), number);
    run.put_claimed(&record).await
}

async fn move_to<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, mut record) = match target(run, args.element.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let at = match point(args.at) {
        Ok(at) => at,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let limit = match limit_of(&record) {
        Ok(limit) => limit,
        Err(refusal) => return Ok(Err(why(refusal))),
    };
    record.recipe = match room_recipe(at, &limit) {
        Ok(recipe) => recipe,
        Err(refusal) => return Ok(Err(why(refusal))),
    };
    if let Err(refusal) = run.buildable(&record).await? {
        return Ok(Err(refusal));
    }
    run.put_claimed(&record).await
}

async fn upper<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, mut record) = match target(run, args.element.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let limit = match limit(run, &args).await? {
        Ok(limit) => limit,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let at = match at_of(&record.recipe) {
        Ok([x, y]) => [Length::from_steps(x), Length::from_steps(y)],
        Err(refusal) => return Ok(Err(why(refusal))),
    };
    // A given height is checked by the record's own rules, as a new room's is.
    if let Limit::Height(height) = &limit
        && height.steps() <= 0
    {
        return Ok(Err(why(RoomError::NoHeight)));
    }
    record.recipe = match room_recipe(at, &limit) {
        Ok(recipe) => recipe,
        Err(refusal) => return Ok(Err(why(refusal))),
    };
    write_limit(&mut record, &limit);
    if let Err(refusal) = above_base(run, &record).await? {
        return Ok(Err(refusal));
    }
    if let Err(refusal) = run.buildable(&record).await? {
        return Ok(Err(refusal));
    }
    run.put_claimed(&record).await
}

async fn delete<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (hash, record) = match target(run, args.element).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(refusal) = run.claim(&record.id).await? {
        return Ok(Err(refusal));
    }
    let pointer = Place::Piece {
        project: run.project.clone(),
        document: run.document.clone(),
        piece: record.id.clone(),
    };
    run.txn.delete(pointer.key()).await?;
    let claim: ClaimChange =
        claims::release(run.txn, run.project, run.document, &record.id).await?;
    Ok(Ok(Applied {
        changes: vec![Change {
            piece: record.id,
            before: Some(hash),
            after: None,
        }],
        claims: vec![claim],
        ..Applied::default()
    }))
}

/// The words for a room command that should have been used.
fn use_instead(what: &str, command: &str) -> String {
    format!("{what} with {command}, which checks it as a room.")
}

/// The part of a generic command's arguments these rules read.
#[derive(Deserialize, Default)]
struct GenericArgs {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    kind: Option<String>,
    recipe: Option<construct_model::geometry::Recipe>,
}

/// The room rules of the generic commands, asked before one runs: a room is made by `room.place`,
/// not `element.create`; it is changed by `room.rename`, `room.number`, `room.move` and
/// `room.upper`, not `element.set` or `element.move`; and deleted by `room.delete`, not
/// `element.delete`. The inner error is the refusal, naming the command to use. An edit these
/// rules have nothing to say of (another command, arguments that do not read, an element that is
/// not there or not a room) is left to the command, which answers it as it always has. Picking a
/// room (`element.claim`, `element.release`) is allowed.
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
    let a_room_recipe = args.recipe.as_ref().is_some_and(is_room_recipe);
    let made = a_room_recipe || args.kind.as_deref() == Some(construct_rooms::ROOM_KIND);
    let cmd = edit.cmd.as_str();
    if cmd == "element.create" && made {
        return Ok(Err(use_instead("A room is placed", "room.place")));
    }
    if !matches!(cmd, "element.set" | "element.move" | "element.delete") {
        return Ok(Ok(()));
    }
    let Some(id) = &args.element else {
        return Ok(Ok(()));
    };
    Ok(
        match records::read_element(txn, project, &edit.target, id).await? {
            Some((_, record)) if is_room(&record) => Err(match cmd {
                "element.delete" => {
                    use_instead(&format!("{} is deleted", record.name), "room.delete")
                }
                "element.move" => use_instead(&format!("{} is moved", record.name), "room.move"),
                _ => format!(
                    "{} is changed with room.rename, room.number, room.move or room.upper, which check it as a room.",
                    record.name
                ),
            }),
            Some(_) if a_room_recipe && cmd == "element.set" => {
                Err(use_instead("A room is placed", "room.place"))
            }
            _ => Ok(()),
        },
    )
}

/// What the page asks of a document's rooms.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct RoomsView {
    /// The next free room number: what a person placing a room is offered.
    pub next_number: String,
}

/// A document's rooms as the page needs them: the next free number.
///
/// # Errors
/// A store error.
pub async fn read_rooms<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<RoomsView, StoreError> {
    let all = records::read_document(reader, project, document).await?;
    let rooms: Vec<Element> = all
        .into_values()
        .map(|(_, record)| record)
        .filter(is_room)
        .collect();
    Ok(RoomsView {
        next_number: next_number(rooms.iter().filter_map(number_of)),
    })
}
