//! The problems a model holds, read-only (job 5.8, CON-A2-F7-P1). There are no commands here: a
//! warning is worked out from the document each time it is asked, never stored, so it is always
//! the list for the document as it stands and undo and redo need nothing of it.
//! [`read_warnings`] reads the walls and rooms the way rooms and the regenerator do
//! ([`construct_regen::rooms`]: a wall's outline as the builders lay it out, a room's outcome from
//! the same search), hands them to construct-warnings' [`warnings`] and returns the list in its
//! order as a view a page, a script or an AI reads: each warning's identity, kind, severity,
//! elements and sentence.
//!
//! A column let go of the grid lines it stood on (they were deleted) is a notice, read from the
//! note its record keeps ([`construct_columns::released_from`]).
//!
//! A dimension's warnings (a reference that is gone or has no place) are measured the way the
//! dimension listing measures them and handed over as they are ([`Model::dimensions`]).
//!
//! A wall that does not build (its type missing, its recipe not as written) is not there, as in
//! every view; a wall with no level is not on one, so it overlaps nothing. A room the search
//! refuses is read as refused, not as a warning of this list (the refusal has its own words on
//! the room's view). The doors, windows and openings hosted in a wall are read as the wall's
//! builder places them ([`construct_regen::inserts`]), each with the stretch of its host its
//! opening takes, so an opening that crosses a wall joined to its host is a warning.

use construct_regen::parameters::At;
use construct_regen::rooms::{WALL_KIND, level_of, of_kind, wall_plan_of};
use construct_rooms::{Outcome, ROOM_KIND};
use construct_warnings::{Insert, Model, ReleasedColumn, Room, RoomState, Wall, warnings};
use kit_core::Id;
use kit_store::{Reader, StoreError};
use serde::Serialize;

/// One warning, as it is read.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct WarningView {
    /// Its kind plus its elements (`kind|id|id`): the same problem keeps it across edits.
    pub identity: String,
    /// Its kind's stable name.
    pub kind: &'static str,
    /// `notice`, `warning` or `error`.
    pub severity: &'static str,
    /// The elements involved, by permanent ID, in the kind's stated order.
    pub elements: Vec<String>,
    /// A plain sentence for people.
    pub words: String,
}

/// What the page asks of a document's warnings: the list now, in order.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct WarningsView {
    /// The warnings, by kind then elements.
    pub warnings: Vec<WarningView>,
}

/// The warnings of a document as it stands now.
///
/// # Errors
/// A store error.
pub async fn read_warnings<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<WarningsView, StoreError> {
    let at = At::Now;
    let mut model = Model::default();
    for wall in of_kind(reader, project, document, WALL_KIND, at).await? {
        let Some(level) = level_of(&wall) else {
            continue;
        };
        let Some(plan) = wall_plan_of(reader, project, document, &wall, at).await? else {
            continue;
        };
        if let Ok(wall) = Wall::new(wall.id.clone(), level, plan.direction, &plan.corners) {
            model.walls.push(wall);
        }
    }
    for insert in construct_regen::inserts::in_document(reader, project, document).await? {
        if let Ok(insert) = Insert::new(
            insert.id,
            insert.who,
            insert.host,
            insert.axis,
            insert.low,
            insert.high,
        ) {
            model.inserts.push(insert);
        }
    }
    for room in of_kind(reader, project, document, ROOM_KIND, at).await? {
        let state =
            match construct_regen::rooms::found_at(reader, project, document, &room, at).await? {
                Ok(Outcome::Enclosed { .. }) => RoomState::Enclosed,
                Ok(Outcome::NotEnclosed) => RoomState::NotEnclosed,
                Ok(Outcome::Redundant { first }) => RoomState::Redundant(first),
                Err(refusal) => RoomState::Refused(refusal.message),
            };
        let who = match construct_rooms::number_of(&room) {
            Some(number) if !number.is_empty() => format!("room {number} {}", room.name),
            _ => format!("room {}", room.name),
        };
        model.rooms.push(Room {
            id: room.id,
            who,
            state,
        });
    }
    for column in of_kind(
        reader,
        project,
        document,
        construct_columns::COLUMN_KIND,
        at,
    )
    .await?
    {
        if let Some(grids) = construct_columns::released_from(&column) {
            model.released_columns.push(ReleasedColumn {
                id: column.id.clone(),
                who: format!("column {}", column.name),
                grids: grids.to_owned(),
            });
        }
    }
    model.dimensions =
        crate::cloud::dimensions::measured_warnings(reader, project, document).await?;
    Ok(WarningsView {
        warnings: warnings(&model)
            .into_iter()
            .map(|w| WarningView {
                identity: w.identity(),
                kind: w.kind,
                severity: w.severity.word(),
                elements: w.elements.iter().map(|e| e.as_str().to_owned()).collect(),
                words: w.words,
            })
            .collect(),
    })
}
