//! The commands that change a document's levels and what is tied to them. They go through the
//! same edit log as `element.*`, with the same access check (`model.edit`) and the same claims:
//! a piece another person is working on is refused, and one person's edits land in log order.
//!
//! Each command reads the document's levels through [`construct_regen::levels`] (the reader the
//! regenerator builds shapes and places with), checks the change with construct-levels' own
//! rules, and writes the changed record. A refusal is the command's result, in construct-levels'
//! words, and changes nothing. Nothing here reads a clock or a random number.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `level.create` | `level`, `name`, `elevation` (typed text), `storey` (yes or no, default yes) | A level record |
//! | `level.rename` | `level`, `name` | The level |
//! | `level.elevation` | `level`, `elevation` (typed text) | The level. Nothing tied to it is written: the regenerator builds what is tied to it again |
//! | `level.storey` | `level`, `storey` | The level |
//! | `level.delete` | `level` | Its pointer goes; refused while anything is tied to it, naming how many |
//! | `level.tie` | `element`, `level`, `offset` (typed text, default `0`) | The element: its base tie, and `level` 0 |
//! | `level.top` | `element`, `level` (or `null`), `offset` (typed text, default `0`) | A wall: its top tie, and its recipe's `height` read from it; `null` unties the top and writes the height it had |
//!
//! A level an old record's integer implies (construct-levels' `implied`) has no record: the
//! rename, elevation and storey commands write its record, in the same entry, and from then on
//! that record is the level. Typed text is read with construct-params' metric display settings
//! until a project keeps its own: `3.2 m`, `-450`, `12.5 mm`. A new element is tied to a level
//! by `element.create`'s `level` argument ([`tie_of_new`]).

use crate::cloud::elements::write_record;
use construct_categories::WALLS;
use construct_levels::{
    LEVEL_KIND, Length, Level, LevelError, Levels, Tie, base_of, implied_id, implied_number,
    parse_length, span, tied_to, top_of, write_base, write_top,
};
use construct_model::element::Element;
use construct_params::Units;
use construct_regen::levels::{self as reader, read_levels};
use construct_regen::parameters::{self as records, At};
use kit_core::Id;
use kit_ops::{Change, Claim, Edit};
use kit_store::{Place, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::Deserialize;

/// Every command here.
pub const COMMANDS: [&str; 7] = [
    "level.create",
    "level.rename",
    "level.elevation",
    "level.storey",
    "level.delete",
    "level.tie",
    "level.top",
];

/// The answer to a command that would make or change a level as an element.
pub const BY_LEVEL_COMMANDS: &str = "Levels are made and changed by the level commands.";

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// The arguments of every command here; each uses some of them.
#[derive(Deserialize, Default)]
struct Args {
    /// A level's ID; for `level.top`, `null` unties the top, so `null` is kept apart from none.
    #[serde(default, deserialize_with = "present")]
    level: Option<serde_json::Value>,
    name: Option<String>,
    elevation: Option<String>,
    storey: Option<bool>,
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    offset: Option<String>,
}

/// An argument that is there, `null` included.
fn present<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<serde_json::Value>, D::Error> {
    serde_json::Value::deserialize(deserializer).map(Some)
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

fn why(error: &LevelError) -> String {
    error.to_string()
}

/// How typed lengths are read: construct-params' metric display settings, until a project keeps
/// its own.
fn units() -> Units {
    Units::default()
}

/// A level's ID from an argument.
fn level_id(value: Option<&serde_json::Value>) -> Result<Id, String> {
    match value {
        Some(serde_json::Value::String(text)) => {
            Id::parse(text).map_err(|_| format!("{text:?} is not a level's ID."))
        }
        _ => Err("This command needs a level.".to_owned()),
    }
}

/// An offset from typed text, `0` when none is given.
fn offset(text: Option<&str>) -> Result<Length, String> {
    match text {
        Some(text) => parse_length(text, &units()).map_err(|e| why(&e)),
        None => Ok(Length::from_steps(0)),
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

    /// The document's levels and records as they are now.
    async fn levels(&mut self) -> Result<Result<(Levels, Vec<Element>), String>, StoreError> {
        read_levels(self.txn, self.project, self.document).await
    }

    /// The hash a level's record replaces: its record's, or `None` for an implied level, whose
    /// record this writes for the first time. Refused if the piece holds something else.
    async fn level_before(&mut self, id: &Id) -> Result<Result<Option<Id>, String>, StoreError> {
        Ok(match self.current(id).await? {
            None => Ok(None),
            Some((hash, record)) if record.kind == LEVEL_KIND => Ok(Some(hash)),
            Some(_) => Err("That element already exists.".to_owned()),
        })
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
        "level.create" => create(&mut run, args).await,
        "level.rename" | "level.elevation" | "level.storey" => change(&mut run, args).await,
        "level.delete" => delete(&mut run, args).await,
        "level.tie" => tie(&mut run, args).await,
        "level.top" => top(&mut run, args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

async fn create<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let made = (|| {
        let id = level_id(args.level.as_ref())?;
        let name = need(args.name, "a name")?;
        let elevation = need(args.elevation, "an elevation")?;
        let elevation = parse_length(&elevation, &units()).map_err(|e| why(&e))?;
        Level::new(id, &name, elevation, args.storey.unwrap_or(true)).map_err(|e| why(&e))
    })();
    let level = match made {
        Ok(level) => level,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(level.id()).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let mut levels = match run.levels().await? {
        Ok((levels, _)) => levels,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let record = level.to_record(run.document);
    if let Err(e) = levels.add(level) {
        return Ok(Err(why(&e)));
    }
    run.put_claimed(&record, None).await
}

/// `level.rename`, `level.elevation` and `level.storey`: each changes one level by
/// construct-levels' rules and writes its record.
async fn change<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let id = match level_id(args.level.as_ref()) {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut levels = match run.levels().await? {
        Ok((levels, _)) => levels,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = match run.edit.cmd.as_str() {
        "level.rename" => need(args.name, "a name")
            .and_then(|name| levels.rename(&id, &name).map_err(|e| why(&e)).cloned()),
        "level.elevation" => need(args.elevation, "an elevation")
            .and_then(|text| parse_length(&text, &units()).map_err(|e| why(&e)))
            .and_then(|elevation| {
                levels
                    .set_elevation(&id, elevation)
                    .map_err(|e| why(&e))
                    .cloned()
            }),
        _ => need(args.storey, "yes or no to being a storey")
            .and_then(|storey| levels.set_storey(&id, storey).map_err(|e| why(&e)).cloned()),
    };
    let level = match changed {
        Ok(level) => level,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let before = match run.level_before(&id).await? {
        Ok(before) => before,
        Err(refusal) => return Ok(Err(refusal)),
    };
    run.put_claimed(&level.to_record(run.document), before)
        .await
}

async fn delete<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let id = match level_id(args.level.as_ref()) {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let (mut levels, all) = match run.levels().await? {
        Ok(read) => read,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let tied = match tied_to(&id, &all) {
        Ok(tied) => tied,
        Err(e) => return Ok(Err(why(&e))),
    };
    if let Err(e) = levels.remove(&id, tied) {
        return Ok(Err(why(&e)));
    }
    let Some((hash, record)) = run.current(&id).await? else {
        return Ok(Err(why(&LevelError::UnknownLevel(id))));
    };
    if record.kind != LEVEL_KIND {
        return Ok(Err(why(&LevelError::UnknownLevel(id))));
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

/// The element a tie command acts on: one with a shape, which a level is not.
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
    if !records::has_shape(&record) {
        return Ok(Err(
            "Types, levels and the parameter table are not tied to levels.".to_owned(),
        ));
    }
    Ok(Ok((hash, record)))
}

/// The level a tie names, as it is now: a level record, or an implied level.
async fn tie_level<T: Txn>(
    run: &mut Run<'_, T>,
    id: &Id,
) -> Result<Result<Level, String>, StoreError> {
    Ok(
        match reader::level_at(run.txn, run.project, run.document, id, At::Now).await? {
            Ok(Some(level)) => Ok(level),
            Ok(None) => Err(why(&LevelError::UnknownLevel(id.clone()))),
            Err(refusal) => Err(refusal),
        },
    )
}

async fn tie<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (hash, mut record) = match target(run, args.element).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let id = match level_id(args.level.as_ref()) {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(refusal) = tie_level(run, &id).await? {
        return Ok(Err(refusal));
    }
    let offset = match offset(args.offset.as_deref()) {
        Ok(offset) => offset,
        Err(refusal) => return Ok(Err(refusal)),
    };
    write_base(&mut record.props, Some(&Tie { level: id, offset }));
    // The tie is the level from now on; the old integer is not written again.
    record.level = 0;
    if let Err(refusal) = top_above_base(run, &record).await? {
        return Ok(Err(refusal));
    }
    run.put_claimed(&record, Some(hash)).await
}

/// Refuses a wall whose top would be at or below its base, by its ties as they are now.
async fn top_above_base<T: Txn>(
    run: &mut Run<'_, T>,
    record: &Element,
) -> Result<Result<(), String>, StoreError> {
    if !reader::reads_top(&record.recipe) {
        return Ok(Ok(()));
    }
    let (Ok(Some(base)), Ok(Some(top))) = (base_of(record), top_of(record)) else {
        return Ok(Err(
            "A wall's top is tied above its base, so it needs both.".to_owned(),
        ));
    };
    let base_level = match tie_level(run, &base.level).await? {
        Ok(level) => level,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let top_level = match tie_level(run, &top.level).await? {
        Ok(level) => level,
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(
        match span((&base_level, base.offset), (&top_level, top.offset)) {
            Ok(height) if height > Length::from_steps(0) => Ok(()),
            Ok(height) => Err(format!(
                "{}'s top would be {height} from its base: a top is above its base.",
                record.name
            )),
            Err(e) => Err(why(&e)),
        },
    )
}

/// The recipe argument a wall's height is.
const HEIGHT: &str = "height";

async fn top<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (hash, mut record) = match target(run, args.element).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if records::category_of(&record.recipe).ok() != Some(WALLS) {
        return Ok(Err("Only a wall's top is tied to a level.".to_owned()));
    }
    if !record.recipe.args.contains_key(HEIGHT) {
        return Ok(Err(format!("{} has no height to tie.", record.name)));
    }
    match args.level {
        Some(serde_json::Value::Null) => {
            // The height it reads now is written back as a number, and the tie goes.
            let placed =
                reader::placed(run.txn, run.project, run.document, &record, At::Now).await?;
            let filled = placed.and_then(|placed| reader::filled(&record.recipe, &placed));
            let height = match filled {
                Ok(recipe) if reader::reads_top(&record.recipe) => recipe.args.get(HEIGHT).cloned(),
                Ok(_) => {
                    return Ok(Err(format!(
                        "{}'s top is not tied to a level.",
                        record.name
                    )));
                }
                Err(failed) => return Ok(Err(failed.message)),
            };
            let Some(height) = height else {
                return Ok(Err(format!("{} has no height to tie.", record.name)));
            };
            record.recipe.args.insert(HEIGHT.to_owned(), height);
            write_top(&mut record.props, None);
        }
        level => {
            let id = match level_id(level.as_ref()) {
                Ok(id) => id,
                Err(refusal) => return Ok(Err(refusal)),
            };
            if let Err(refusal) = tie_level(run, &id).await? {
                return Ok(Err(refusal));
            }
            let offset = match offset(args.offset.as_deref()) {
                Ok(offset) => offset,
                Err(refusal) => return Ok(Err(refusal)),
            };
            write_top(&mut record.props, Some(&Tie { level: id, offset }));
            record
                .recipe
                .args
                .insert(HEIGHT.to_owned(), reader::top_arg());
            if let Err(refusal) = top_above_base(run, &record).await? {
                return Ok(Err(refusal));
            }
        }
    }
    run.put_claimed(&record, Some(hash)).await
}

/// The tie of a new element, from `element.create`'s `level` argument: a level's ID, or the
/// integer an old page sends, which names its implied level; absent, `null` or `0` is no level.
/// The level must be in the model: a level record, or an implied level.
///
/// # Errors
/// A store error; the inner error is the refusal, in words.
pub async fn tie_of_new<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    level: Option<&serde_json::Value>,
) -> Result<Result<Option<Tie>, String>, StoreError> {
    let at_zero = |level: Id| {
        Some(Tie {
            level,
            offset: Length::from_steps(0),
        })
    };
    Ok(match level {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => {
            match n.as_i64().and_then(|n| i32::try_from(n).ok()) {
                Some(n) => Ok(implied_id(n).and_then(at_zero)),
                None => Err(format!("{n} is not a level.")),
            }
        }
        Some(serde_json::Value::String(text)) => {
            let Ok(id) = Id::parse(text) else {
                return Ok(Err(format!("{text:?} is not a level's ID.")));
            };
            let stored = records::read_element(txn, project, document, &id).await?;
            match stored {
                Some((_, record)) if record.kind == LEVEL_KIND => Ok(at_zero(id)),
                _ if implied_number(&id).is_some() => Ok(at_zero(id)),
                _ => Err(why(&LevelError::UnknownLevel(id))),
            }
        }
        Some(_) => Err("A level is named by its ID.".to_owned()),
    })
}

/// Whether every tie of the records an undo wrote names a level that is in the model: a level
/// record, or an implied level. `whole`: a level, a type or the parameter table was written or
/// taken away, so every element of the document is checked.
///
/// # Errors
/// A store error; the inner error names the element and the level it would be tied to.
pub async fn ties_hold<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    written: &[Element],
    whole: bool,
) -> Result<Result<(), String>, StoreError> {
    let all: Vec<Element> = if whole {
        records::read_document(txn, project, document)
            .await?
            .into_values()
            .map(|(_, record)| record)
            .collect()
    } else {
        written.to_vec()
    };
    for record in all.iter().filter(|record| records::has_shape(record)) {
        let ties = match (base_of(record), top_of(record)) {
            (Ok(base), Ok(top)) => [base, top],
            (Err(e), _) | (_, Err(e)) => return Ok(Err(why(&e))),
        };
        for tie in ties.into_iter().flatten() {
            let found = reader::level_at(txn, project, document, &tie.level, At::Now).await?;
            match found {
                Ok(Some(_)) => {}
                Ok(None) => {
                    return Ok(Err(format!(
                        "{} would be tied to level {}, which is no longer in this model.",
                        record.name, tie.level
                    )));
                }
                Err(refusal) => return Ok(Err(refusal)),
            }
        }
    }
    Ok(Ok(()))
}
