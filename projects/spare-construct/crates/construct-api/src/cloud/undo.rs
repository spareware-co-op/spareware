//! Undo and redo through the edit log (CON-A2-F5-P3; ARCHITECTURE.md: "undo and redo step
//! through the operation log"; note A05, which K08 builds).
//!
//! An undo is an ordinary command, `edit.undo`, in the same log as every edit: the orderer runs
//! it in order, behind the same access check (`model.edit`) and the same claims, and everyone
//! watching gets it as an ordinary entry. It reverses one earlier step of the same person in the
//! same document, built from what that step's entry recorded: each piece's record before and
//! after, by content ID. Records are named by their content and never deleted, so a step's
//! before-records are always there. `edit.redo` reverses the undo, so an undo of an undo is a redo.
//! Nothing is ever removed from the log. Neither takes arguments (`{}`): which step is found from
//! the log, by the rule below, the same way on every run.
//!
//! **Steps.** A person's steps in a document are their entries there that applied, of the
//! commands in [`is_step`], and changed a record: a move to where an element already is (the page
//! sends one when an element is clicked to select it) is no step. One entry is one step, however many records it wrote: a
//! `parameter.bind` that rewrote the table and every type of its categories is undone at once. A
//! run of `element.move` entries of one element, with no other entry of that person in that
//! document between, is one step: a drag sends a move every few milliseconds after the claim it
//! starts with, and the claim of the next drag ends the run.
//!
//! **Order.** Each person has their own. Ordinary steps and redos go on their undo order, and
//! an undo takes the last off; undos go on their redo order, a redo takes the last off, and an
//! ordinary step clears it. Another person's entries change neither. The order is never kept
//! anywhere: [`find`] reads it back from the log, from the newest entry down, so a server that
//! starts again, or the log run again on an empty store, gives the same answer.
//!
//! **Reversing a step** puts each piece it changed back as it was before, column by column (note
//! A05, "stale writes, column by column"): the columns are `kind`, `name`, `level`, `placement`,
//! `recipe`, `references` and each `props` key, compared exactly. A column the step did not change
//! keeps what it holds now, so other people's changes to other columns stand. A column the step
//! changed goes back if it still holds what the step left (or already holds what it had). If
//! anyone else has changed it since, the undo is turned back, `"<name> has changed <record> since."`;
//! a piece someone deleted since, or one the step made that someone has changed since, is the
//! same. Before that, another person's live claim on any piece it would write refuses it with
//! the claim's own words, `"<name> is working on this."`; it then takes the claim on each piece it
//! writes and lets go of each it deletes, as `element.set` and `element.delete` do. Last, every
//! record it wrote must read under the document's rules (its type exists, its values read), and
//! when it wrote a type or the parameter table every record of the document is read again; a
//! failure is `"That can't be undone: "` and the reader's own words. So is leaving an element
//! without its host: one it writes whose host is gone, or one hosted on a piece it deletes
//! ([`crate::cloud::hosting::undo_rules`]), or a placed family without the version of its family
//! it names ([`crate::cloud::loading::undo_rules`]). Every tie of what it wrote must name a level
//! still in the model, and when it wrote or took away a level, every element of the document is
//! checked so (`levels::ties_hold`), and a view it wrote must still stand on its levels, phases
//! and options ([`crate::cloud::views::undo_rules`]).
//!
//! **All or nothing** (CON-A2-F5-P2, A05 "that command is turned back"): an undo is one command,
//! so any of those refusals turns the whole undo back and none of its pieces is written
//! ([`crate::cloud::transaction`]). A turned-back undo is in the log as such, and is no step.
//!
//! Not steps, never undone and never in the way: `element.claim`, `element.release`,
//! `document.checkpoint`, and the folder, package and link commands, which change no element of a
//! document. No step command is refused for what it is; every refusal is found when the undo
//! runs. Nothing here reads a clock or a random number: the claim's time is the orderer's.

use crate::cloud::elements::write_record;
use crate::cloud::{
    families, floors, grids, hosting, levels, loading, params, roofs, views, walls,
};
use construct_model::element::Element;
use construct_regen::parameters::{self as records, TABLE_KIND, TYPE_KIND};
use construct_types::Types;
use kit_core::Id;
use kit_ops::{Change, Claim, ClaimChange, Edit, Entry};
use kit_store::{Place, Reader, Scope, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context, Outcome, claims, read_log};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The two commands here.
pub const COMMANDS: [&str; 2] = [UNDO, REDO];
/// Reverses the person's last step.
pub const UNDO: &str = "edit.undo";
/// Reverses the person's last undo.
pub const REDO: &str = "edit.redo";
/// The answer when a person has no step to undo in the document.
pub const NOTHING_TO_UNDO: &str = "There is nothing of yours to undo in this model.";
/// The answer when a person has no undo to redo in the document.
pub const NOTHING_TO_REDO: &str = "There is nothing of yours to redo in this model.";
/// How a refusal by the document's own rules begins.
pub const CANNOT: &str = "That can't be undone: ";

/// The element commands that are steps; the parameter and type commands and these two are too.
const ELEMENT_STEPS: [&str; 4] = [
    "element.create",
    "element.move",
    "element.set",
    "element.delete",
];
/// The move command, whose runs are one step.
const MOVE: &str = "element.move";
/// How many log entries are read at a time, going back.
const WINDOW: u64 = 128;

/// Whether a command is one of these two.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// Whether an applied entry of this command is a step.
#[must_use]
pub fn is_step(cmd: &str) -> bool {
    ELEMENT_STEPS.contains(&cmd)
        || params::is_command(cmd)
        || grids::is_command(cmd)
        || hosting::is_command(cmd)
        || families::is_command(cmd)
        || levels::is_command(cmd)
        || walls::is_command(cmd)
        || floors::is_command(cmd)
        || roofs::is_command(cmd)
        || views::is_command(cmd)
        || is_command(cmd)
}

/// A step a person could undo or redo now: the newest entry of it, and its command.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// The entry's log number: for an undo, the step's last entry; for a redo, the undo's.
    pub seq: u64,
    /// That entry's command.
    pub cmd: String,
}

/// What a person can undo and redo in a document now.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// The step an undo would reverse, if any.
    pub undo: Option<Step>,
    /// The undo a redo would reverse, if any.
    pub redo: Option<Step>,
}

/// Which of the person's orders to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    /// Their undo order: the step an undo reverses.
    Undo,
    /// Their redo order: the undo a redo reverses.
    Redo,
}

/// The entries to reverse, and who last changed each piece after them.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// The step's entries, in log order: one, or a run of moves.
    pub entries: Vec<Entry>,
    /// For each piece, the author of the newest later entry by someone else that changed it.
    pub changed_by: BTreeMap<Id, Id>,
}

/// A run of moves of one element, open while reading back.
struct Run {
    element: Id,
    /// Whether the run is the step being looked for, or one an undo has already taken.
    wanted: bool,
}

/// Whether an entry is a change at all: an undo or a redo always is, so each person's orders stay
/// in step; any other entry only if it changed a record. Clicking an element to select it sends a
/// move to where it already is, which changes nothing and is no step.
fn changed_something(entry: &Entry) -> bool {
    is_command(&entry.cmd)
        || entry
            .changes
            .iter()
            .any(|change| change.before != change.after)
}

/// The one piece a move changed.
fn moved(entry: &Entry) -> Option<&Id> {
    if entry.cmd == MOVE {
        entry.changes.first().map(|change| &change.piece)
    } else {
        None
    }
}

/// Reads one of a person's orders in a document back from the log, from entry `head` down:
/// the step (or undo) it would reverse now, or `None`.
///
/// # Errors
/// A [`StoreError`] when the store fails or an entry cannot be read.
pub async fn find<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    person: &Id,
    head: u64,
    which: Which,
) -> Result<Option<Found>, StoreError> {
    let scope = Scope::Project(project.clone());
    let mut depth: u64 = 0;
    let mut run: Option<Run> = None;
    let mut taken: Vec<Entry> = Vec::new();
    let mut changed_by: BTreeMap<Id, Id> = BTreeMap::new();
    let mut to = head;
    while to > 0 {
        let from = to.saturating_sub(WINDOW);
        let entries = read_log(reader, &scope, from, to).await?;
        to = from;
        for entry in entries.into_iter().rev() {
            if entry.target != *document || entry.failed.is_some() {
                continue;
            }
            if entry.author != *person {
                for change in &entry.changes {
                    changed_by
                        .entry(change.piece.clone())
                        .or_insert_with(|| entry.author.clone());
                }
                continue;
            }
            if let Some(open) = &run {
                if moved(&entry) == Some(&open.element) {
                    if open.wanted {
                        taken.insert(0, entry);
                    }
                    continue;
                }
                if open.wanted {
                    return Ok(Some(Found {
                        entries: taken,
                        changed_by,
                    }));
                }
                run = None;
            }
            if !is_step(&entry.cmd) || !changed_something(&entry) {
                continue;
            }
            match (which, entry.cmd.as_str()) {
                (Which::Undo, UNDO) | (Which::Redo, REDO) => depth = depth.saturating_add(1),
                (Which::Redo, UNDO) | (Which::Undo, _) if depth > 0 => {
                    depth = depth.saturating_sub(1);
                    if let Some(element) = moved(&entry) {
                        run = Some(Run {
                            element: element.clone(),
                            wanted: false,
                        });
                    }
                }
                (Which::Redo, UNDO) | (Which::Undo, _) => {
                    if let Some(element) = moved(&entry) {
                        run = Some(Run {
                            element: element.clone(),
                            wanted: true,
                        });
                        taken.push(entry);
                    } else {
                        return Ok(Some(Found {
                            entries: vec![entry],
                            changed_by,
                        }));
                    }
                }
                // An ordinary step clears the redo order.
                (Which::Redo, _) => return Ok(None),
            }
        }
    }
    Ok((!taken.is_empty()).then_some(Found {
        entries: taken,
        changed_by,
    }))
}

/// What a person can undo and redo in a document now, read from the log as the commands read it.
///
/// # Errors
/// A [`StoreError`] when the store fails or an entry cannot be read.
pub async fn state<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    person: &Id,
) -> Result<State, StoreError> {
    let scope = Scope::Project(project.clone());
    let head: u64 = read_row(reader, &scope.head()).await?.unwrap_or(0);
    let step = |found: Option<Found>| {
        found
            .and_then(|found| found.entries.last().cloned())
            .map(|entry| Step {
                seq: entry.seq,
                cmd: entry.cmd,
            })
    };
    let undo = find(reader, project, document, person, head, Which::Undo).await?;
    let redo = find(reader, project, document, person, head, Which::Redo).await?;
    Ok(State {
        undo: step(undo),
        redo: step(redo),
    })
}

/// The part of a person's row a claim and a refusal need.
#[derive(Deserialize)]
struct Named {
    name: String,
}

async fn name_of<R: Reader>(reader: &mut R, person: &Id) -> Result<String, StoreError> {
    let row: Option<Named> = read_row(reader, &Place::Person(person.clone()).key()).await?;
    Ok(row.map_or_else(|| person.to_string(), |row| row.name))
}

async fn record_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    hash: Option<&Id>,
) -> Result<Option<Element>, StoreError> {
    let Some(hash) = hash else {
        return Ok(None);
    };
    let place = Place::Record {
        project: project.clone(),
        hash: hash.clone(),
    };
    let record: Option<Element> = read_row(reader, &place.key()).await?;
    record
        .ok_or_else(|| StoreError::Damaged(format!("the record {hash} is missing")))
        .map(Some)
}

/// One column, three ways: `base` what the step left, `target` what it had, `current` what it
/// holds now. `false` when someone else has changed it since.
fn column<T: PartialEq + Clone>(base: &T, target: &T, current: &T, out: &mut T) -> bool {
    if target == base {
        return true;
    }
    if current == base || current == target {
        target.clone_into(out);
        return true;
    }
    false
}

/// A record put back column by column: every column the step changed back to `target`, every
/// other as it is now. `None` when a column the step changed has been changed since.
#[must_use]
pub fn merged(base: &Element, target: &Element, current: &Element) -> Option<Element> {
    let mut out = current.clone();
    let mut fine = column(&base.kind, &target.kind, &current.kind, &mut out.kind);
    fine &= column(&base.name, &target.name, &current.name, &mut out.name);
    fine &= column(&base.level, &target.level, &current.level, &mut out.level);
    fine &= column(
        &base.placement,
        &target.placement,
        &current.placement,
        &mut out.placement,
    );
    fine &= column(
        &base.recipe,
        &target.recipe,
        &current.recipe,
        &mut out.recipe,
    );
    fine &= column(
        &base.references,
        &target.references,
        &current.references,
        &mut out.references,
    );
    let keys: BTreeSet<&String> = base
        .props
        .keys()
        .chain(target.props.keys())
        .chain(current.props.keys())
        .collect();
    for key in keys {
        let (b, t, c) = (
            base.props.get(key),
            target.props.get(key),
            current.props.get(key),
        );
        if t == b {
            continue;
        }
        if c == b || c == t {
            match t {
                Some(value) => out.props.insert(key.clone(), value.clone()),
                None => out.props.remove(key),
            };
        } else {
            fine = false;
        }
    }
    fine.then_some(out)
}

/// One piece of a step: its record before the step and after it, by content ID.
struct Piece {
    id: Id,
    before: Option<Id>,
    after: Option<Id>,
}

/// The pieces a step changed, in the order it first changed them: before from its first entry
/// that changed each, after from its last.
fn pieces_of(entries: &[Entry]) -> Vec<Piece> {
    let mut pieces: Vec<Piece> = Vec::new();
    for change in entries.iter().flat_map(|entry| &entry.changes) {
        match pieces.iter_mut().find(|piece| piece.id == change.piece) {
            Some(piece) => piece.after.clone_from(&change.after),
            None => pieces.push(Piece {
                id: change.piece.clone(),
                before: change.before.clone(),
                after: change.after.clone(),
            }),
        }
    }
    pieces
}

/// What a piece will be: its current hash, and the record it becomes (`None`: it goes).
struct Planned {
    id: Id,
    current: Option<Id>,
    next: Option<Element>,
    /// Whether it is, or was, a type, the parameter table or a level.
    not_an_element: bool,
}

/// Runs `edit.undo` or `edit.redo`.
///
/// # Errors
///
/// A store error; an undo that cannot be made is the outcome's error instead.
pub async fn apply<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Context<'_>,
    edit: &Edit,
    claim_ms: u64,
) -> Result<Outcome, StoreError> {
    let document = &edit.target;
    let which = if edit.cmd == REDO {
        Which::Redo
    } else {
        Which::Undo
    };
    let Some(found) = find(txn, project, document, &edit.person, context.head, which).await? else {
        let nothing = match which {
            Which::Undo => NOTHING_TO_UNDO,
            Which::Redo => NOTHING_TO_REDO,
        };
        return Ok(Err(nothing.to_owned()));
    };
    let plan = match plan(txn, project, context, edit, &found).await? {
        Ok(plan) => plan,
        Err(refusal) => return Ok(Err(refusal)),
    };

    // Written in the staged transaction, so a refusal below leaves nothing.
    let name = name_of(txn, &edit.person).await?;
    let mut changes = Vec::new();
    let mut taken: Vec<ClaimChange> = Vec::new();
    let mut whole = false;
    let mut written = Vec::new();
    let mut deleted = Vec::new();
    for planned in plan {
        whole |= planned.not_an_element;
        let pointer = Place::Piece {
            project: project.clone(),
            document: document.clone(),
            piece: planned.id.clone(),
        };
        let after = if let Some(record) = &planned.next {
            let hash = write_record(txn, project, record).await?;
            if planned.current.as_ref() == Some(&hash) {
                continue;
            }
            txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
            let mine = Claim {
                person: edit.person.clone(),
                name: name.clone(),
                // milliseconds since 1970 plus a claim's length: far below u64::MAX
                until_ms: context.now_ms.saturating_add(claim_ms),
            };
            taken.push(claims::take(txn, project, document, &planned.id, mine).await?);
            written.push(record.clone());
            Some(hash)
        } else {
            txn.delete(pointer.key()).await?;
            taken.push(claims::release(txn, project, document, &planned.id).await?);
            deleted.push(planned.id.clone());
            None
        };
        changes.push(Change {
            piece: planned.id,
            before: planned.current,
            after,
        });
    }

    if let Err(why) = check(txn, project, document, &written, whole).await? {
        return Ok(Err(format!("{CANNOT}{why}")));
    }
    if let Err(why) = hosting::undo_rules(txn, project, document, &written, &deleted).await? {
        return Ok(Err(format!("{CANNOT}{why}")));
    }
    // Nor a placed family without the version of its family it names.
    if let Err(why) = loading::undo_rules(txn, project, document, &written, &deleted).await? {
        return Ok(Err(format!("{CANNOT}{why}")));
    }
    // An undo may not leave an element tied to a level that is no longer in the model.
    if let Err(why) = levels::ties_hold(txn, project, document, &written, whole).await? {
        return Ok(Err(format!("{CANNOT}{why}")));
    }
    // Nor a view without the levels, phases or options it stands on.
    if let Err(why) = views::undo_rules(txn, project, document, &written, whole).await? {
        return Ok(Err(format!("{CANNOT}{why}")));
    }
    Ok(Ok(Applied {
        changes,
        claims: taken,
        ..Applied::default()
    }))
}

/// What each piece of the step becomes, or why the undo is turned back: another person's claim,
/// or a column someone else has changed since.
async fn plan<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Context<'_>,
    edit: &Edit,
    found: &Found,
) -> Result<Result<Vec<Planned>, String>, StoreError> {
    let document = &edit.target;
    let pieces = pieces_of(&found.entries);

    // Another person's claim refuses it, as it refuses any edit.
    for piece in &pieces {
        let holder = claims::held_by_another(
            txn,
            project,
            document,
            &piece.id,
            &edit.person,
            context.now_ms,
        )
        .await?;
        if let Some(claim) = holder {
            return Ok(Err(format!("{} is working on this.", claim.name)));
        }
    }

    // Each piece back as it was, column by column, unless someone else has changed it since.
    let mut plan = Vec::new();
    for piece in pieces {
        let base = record_of(txn, project, piece.after.as_ref()).await?;
        let target = record_of(txn, project, piece.before.as_ref()).await?;
        let now = records::read_element(txn, project, document, &piece.id).await?;
        let next = match (&target, &base, &now) {
            (None, None, _) => continue,
            (None, Some(base), Some((_, current))) if current == base => Some(None),
            (Some(target), None, None) => Some(Some(target.clone())),
            (Some(target), Some(base), Some((_, current))) => {
                merged(base, target, current).map(Some)
            }
            _ => None,
        };
        let Some(next) = next else {
            let label = base
                .as_ref()
                .or(target.as_ref())
                .map_or_else(|| piece.id.to_string(), |record| record.name.clone());
            let refusal = match found.changed_by.get(&piece.id) {
                Some(author) => {
                    format!("{} has changed {label} since.", name_of(txn, author).await?)
                }
                None => format!("{label} has changed since."),
            };
            return Ok(Err(refusal));
        };
        let kind = |record: &Element| {
            record.kind == TYPE_KIND
                || record.kind == TABLE_KIND
                || record.kind == construct_levels::LEVEL_KIND
        };
        let not_an_element = base.as_ref().is_some_and(kind) || target.as_ref().is_some_and(kind);
        let current = now.map(|(hash, _)| hash);
        plan.push(Planned {
            id: piece.id,
            current,
            next,
            not_an_element,
        });
    }

    Ok(Ok(plan))
}

/// Whether the records written read under the document's rules: each element's type exists and
/// its values read against it. `whole`: a type or the parameter table was written, so every
/// record of the document is read again, the types together.
async fn check<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    written: &[Element],
    whole: bool,
) -> Result<Result<(), String>, StoreError> {
    if let Err(why) = grids::check(txn, project, document, written).await? {
        return Ok(Err(why));
    }
    let context = match records::read_context(txn, project, document).await? {
        Ok(context) => context,
        Err(why) => return Ok(Err(why)),
    };
    let reads = |record: &Element, types: &Types| -> Result<(), String> {
        match records::type_of(record) {
            Some(type_id) => {
                let ty = types
                    .get(&type_id)
                    .ok_or_else(|| format!("no type has id {type_id}"))?;
                context.instance(record, ty).map(|_| ())
            }
            None => context.own_values(record, None).map(|_| ()),
        }
    };
    if whole {
        let all = records::read_document(txn, project, document).await?;
        let type_records: Vec<Element> = all
            .values()
            .map(|(_, record)| record)
            .filter(|record| record.kind == TYPE_KIND)
            .cloned()
            .collect();
        let types = match Types::from_records(&type_records, &context.params, &context.categories) {
            Ok(types) => types,
            Err(e) => return Ok(Err(e.to_string())),
        };
        for (_, record) in all.values() {
            if records::has_shape(record)
                && let Err(why) = reads(record, &types)
            {
                return Ok(Err(why));
            }
        }
        return Ok(Ok(()));
    }
    for record in written.iter().filter(|record| records::has_shape(record)) {
        let mut types = Types::new();
        if let Some(type_id) = records::type_of(record) {
            match records::read_type(txn, project, document, &type_id, &context).await? {
                Ok(ty) => {
                    if let Err(e) = types.add(ty) {
                        return Ok(Err(e.to_string()));
                    }
                }
                Err(why) => return Ok(Err(why)),
            }
        }
        if let Err(why) = reads(record, &types) {
            return Ok(Err(why));
        }
    }
    Ok(Ok(()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;
    use construct_model::element::Placement;
    use construct_model::geometry::Recipe;

    fn wall(name: &str, x: i64, note: Option<&str>) -> Element {
        let mut props = BTreeMap::new();
        if let Some(note) = note {
            props.insert("note".to_owned(), note.to_owned());
        }
        Element {
            id: Id::parse("W-1").unwrap(),
            document: Id::parse("doc").unwrap(),
            kind: "wall".to_owned(),
            name: name.to_owned(),
            level: 1,
            placement: Placement { x, y: 0, z: 0 },
            recipe: Recipe::construct("wall.straight", BTreeMap::new()),
            props,
            references: Vec::new(),
        }
    }

    #[test]
    fn a_column_the_step_changed_goes_back_and_others_stay() {
        // The step renamed it; since then someone moved it and added a note.
        let (target, base) = (wall("A", 0, None), wall("B", 0, None));
        let current = wall("B", 500, Some("theirs"));
        assert_eq!(
            merged(&base, &target, &current),
            Some(wall("A", 500, Some("theirs")))
        );
        // Already as it was: nothing to fight over.
        assert_eq!(
            merged(&base, &target, &wall("A", 0, None)),
            Some(wall("A", 0, None))
        );
        // Renamed again since by someone else: turned back.
        assert_eq!(merged(&base, &target, &wall("C", 0, None)), None);
        // A key the step added is taken away again; one it took away comes back.
        let noted = wall("A", 0, Some("mine"));
        assert_eq!(
            merged(&noted, &wall("A", 0, None), &wall("A", 9, Some("mine"))),
            Some(wall("A", 9, None))
        );
        assert_eq!(
            merged(&wall("A", 0, None), &noted, &wall("A", 9, None)),
            Some(wall("A", 9, Some("mine")))
        );
    }
}
