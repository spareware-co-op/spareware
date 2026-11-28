//! Spare Construct: warnings as data, the problems a model holds (CON-A2-F7-P1).
//!
//! A warning is a plain value: its kind (a stable name), how serious it is, the elements involved
//! (their permanent IDs, in a stated order) and a sentence for people. People, AI and scripts read
//! the same list. Map: projects/spare-construct/ARCHITECTURE.md; the decisions are in
//! notes/CON-A2-F7-P1-warnings.md and the read-only form in FORMAT.md beside this crate.
//!
//! - [`warnings`]: the one function. It takes a [`Model`], the elements of a document as they
//!   stand (its walls as [`Wall`]s, its rooms as [`Room`]s, the doors and windows hosted in its
//!   walls as [`Insert`]s), and gives the warnings now, in a
//!   stated order. Nothing is stored: the list is worked out each time it is asked, so after any
//!   edit it is the new list and a warning can never be out of date. A warning's identity
//!   ([`Warning::identity`]) is its kind plus its elements, so the same problem has the same
//!   identity across edits that do not touch it.
//! - [`RULES`]: the kinds, one entry each ([`Rule`]): its name, its [`Severity`] and the function
//!   that finds its warnings. A new kind is one more entry in the table and one function.
//! - [`wall`]: [`Wall`], an outline with a direction on a level, [`overlap_along_length`], the
//!   exact test of two of them, and [`crosses_stretch`], the exact test of a joined wall against
//!   the stretch of its host an insert's opening takes.
//!
//! The order: by kind name, then by the elements' IDs, ascending, and a warning's own elements in
//! ascending order of ID unless its kind says otherwise (a kind's doc says). The same model gives
//! the same list, byte for byte, on every machine.
//!
//! What a person decides about a warning (accepting one) is not in the feature text, so nothing is
//! stored here; if that comes, it is a command in the edit log keyed by a warning's identity.
//!
//! Depends on kit-core only. The store is read by construct-api's `cloud::warnings`, which builds
//! the [`Model`] from the document with the same readers the regenerator uses for walls and
//! rooms; this crate does geometry on whole numbers only, and invents no tolerance.

pub mod wall;

pub use wall::{LIMIT, MAX_CORNERS, Wall, WallError, crosses_stretch, overlap_along_length};

use kit_core::Id;

/// How serious a warning is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Worth knowing; nothing is wrong yet.
    Notice,
    /// A problem a person should look at; the model still works.
    Warning,
    /// The model cannot be trusted until it is fixed.
    Error,
}

impl Severity {
    /// The word it is read by: `notice`, `warning` or `error`.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Notice => "notice",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// One problem in a model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    /// Its kind's stable name, such as `room.not_enclosed`.
    pub kind: &'static str,
    /// How serious it is.
    pub severity: Severity,
    /// The elements involved, by permanent ID, in the order its kind states.
    pub elements: Vec<Id>,
    /// A plain sentence for people.
    pub words: String,
}

impl Warning {
    /// Its identity: its kind and its elements, as `kind|id|id`. The same problem has the same
    /// identity for as long as it lasts, however the rest of the model changes.
    #[must_use]
    pub fn identity(&self) -> String {
        let mut text = self.kind.to_owned();
        for element in &self.elements {
            text.push('|');
            text.push_str(element.as_str());
        }
        text
    }
}

/// What became of a room when its loop was looked for among the walls of its level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoomState {
    /// Its walls close round its point.
    Enclosed,
    /// They do not.
    NotEnclosed,
    /// Another room, whose ID comes first, is in the same loop.
    Redundant(Id),
    /// The search refused it, in words.
    Refused(String),
}

/// A room, as the warnings see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Room {
    /// Its element ID.
    pub id: Id,
    /// How people name it in a sentence: `room 101 Kitchen`.
    pub who: String,
    /// What became of it.
    pub state: RoomState,
}

/// Why a stretch cannot be taken as an insert's opening.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertError {
    /// The axis is not 0 (x) or 1 (y).
    Axis,
    /// The stretch has no length: it does not run from a lower to a higher number.
    Empty,
    /// A number beyond the kernel's grid ([`LIMIT`]).
    TooBig,
}

impl std::fmt::Display for InsertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Axis => "an insert's axis is 0 (x) or 1 (y)",
            Self::Empty => "an insert's opening has a length",
            Self::TooBig => "a coordinate is beyond the kernel's grid",
        })
    }
}

impl std::error::Error for InsertError {}

/// A door or window hosted in a wall, as the warnings see it: the stretch its opening takes along
/// its host wall's line, in plan, in the model's grid steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Insert {
    id: Id,
    who: String,
    host: Id,
    axis: usize,
    low: i64,
    high: i64,
}

impl Insert {
    /// The insert `id`, called `who` in a sentence (`window W-1`), hosted in the wall `host`,
    /// whose line runs along `axis` (0 for x, 1 for y): its opening takes the stretch from `low`
    /// to `high` along that axis, in plan.
    ///
    /// # Errors
    /// [`InsertError`] for an axis that is neither, a stretch with no length, or a coordinate
    /// beyond the grid.
    pub fn new(
        id: Id,
        who: String,
        host: Id,
        axis: usize,
        low: i64,
        high: i64,
    ) -> Result<Self, InsertError> {
        if axis > 1 {
            return Err(InsertError::Axis);
        }
        if low >= high {
            return Err(InsertError::Empty);
        }
        let beyond = |v: i64| v.unsigned_abs() > LIMIT.unsigned_abs();
        if beyond(low) || beyond(high) {
            return Err(InsertError::TooBig);
        }
        Ok(Self {
            id,
            who,
            host,
            axis,
            low,
            high,
        })
    }
}

/// The elements of a document the warnings read, as they stand now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Model {
    /// Every wall that builds, with its level.
    pub walls: Vec<Wall>,
    /// Every room, with what became of it.
    pub rooms: Vec<Room>,
    /// What the dimensions found, already measured: construct-dimensions' `measure` works each
    /// dimension out and its warnings are given here as they are (this crate depends on kit-core
    /// only, so it cannot measure). [`RULES`] lists their kinds; a warning of a kind that is not
    /// in [`RULES`] is not listed.
    pub dimensions: Vec<Warning>,
    /// Every door and window hosted in a wall, with the stretch its opening takes.
    pub inserts: Vec<Insert>,
}

/// One kind of warning.
pub struct Rule {
    /// Its stable name.
    pub kind: &'static str,
    /// How serious its warnings are.
    pub severity: Severity,
    /// Finds this kind's warnings in a model, in any order ([`warnings`] sorts them).
    pub find: fn(&Rule, &Model) -> Vec<Warning>,
}

impl Rule {
    fn warn(&self, elements: Vec<Id>, words: String) -> Warning {
        Warning {
            kind: self.kind,
            severity: self.severity,
            elements,
            words,
        }
    }
}

/// Every kind there is. Add a kind here, with its function; nothing else changes.
pub const RULES: [Rule; 7] = [
    Rule {
        kind: "dimension.out_of_range",
        severity: Severity::Warning,
        find: measured_dimensions,
    },
    Rule {
        kind: "dimension.reference_missing",
        severity: Severity::Warning,
        find: measured_dimensions,
    },
    Rule {
        kind: "dimension.reference_parallel",
        severity: Severity::Warning,
        find: measured_dimensions,
    },
    Rule {
        kind: "room.not_enclosed",
        severity: Severity::Warning,
        find: not_enclosed,
    },
    Rule {
        kind: "room.redundant",
        severity: Severity::Warning,
        find: redundant,
    },
    Rule {
        kind: "wall.overlap",
        severity: Severity::Warning,
        find: overlapping,
    },
    Rule {
        kind: "insert.crosses_joined_wall",
        severity: Severity::Warning,
        find: crossing_joined,
    },
];

/// A reference of a dimension that cannot be used (it is gone, it has no place in the view, it
/// runs along the dimension's line, or it lies off the kernel's grid), as construct-dimensions
/// measured it, one warning for each. Its elements: the dimension, then the element referenced.
fn measured_dimensions(rule: &Rule, model: &Model) -> Vec<Warning> {
    model
        .dimensions
        .iter()
        .filter(|found| found.kind == rule.kind)
        .map(|found| rule.warn(found.elements.clone(), found.words.clone()))
        .collect()
}

/// A room whose walls do not close round its point. Its elements: the room.
fn not_enclosed(rule: &Rule, model: &Model) -> Vec<Warning> {
    model
        .rooms
        .iter()
        .filter(|room| room.state == RoomState::NotEnclosed)
        .map(|room| {
            rule.warn(
                vec![room.id.clone()],
                format!(
                    "{} is not enclosed: the walls round its point do not close",
                    capital(&room.who)
                ),
            )
        })
        .collect()
}

/// A room placed in a space another room already holds: its loop is the same as the room whose
/// ID comes first, so it has no area of its own. Its elements, in this order: the redundant room,
/// then the room that keeps the space (named by its ID where the model does not list it).
fn redundant(rule: &Rule, model: &Model) -> Vec<Warning> {
    model
        .rooms
        .iter()
        .filter_map(|room| match &room.state {
            RoomState::Redundant(first) => Some((room, first)),
            _ => None,
        })
        .map(|(room, first)| {
            let keeper = model
                .rooms
                .iter()
                .find(|other| other.id == *first)
                .map_or_else(|| format!("room {}", first.as_str()), |r| r.who.clone());
            rule.warn(
                vec![room.id.clone(), first.clone()],
                format!(
                    "{} is redundant: {keeper} already holds the same enclosed space",
                    capital(&room.who)
                ),
            )
        })
        .collect()
}

/// Two walls on one level that overlap along their length ([`overlap_along_length`]). Its
/// elements: the two walls, the lower ID first.
fn overlapping(rule: &Rule, model: &Model) -> Vec<Warning> {
    let mut found = Vec::new();
    for (at, a) in model.walls.iter().enumerate() {
        for b in model.walls.iter().skip(at.saturating_add(1)) {
            if a.id() != b.id() && overlap_along_length(a, b) {
                let (first, second) = if a.id() <= b.id() { (a, b) } else { (b, a) };
                found.push(rule.warn(
                    vec![first.id().clone(), second.id().clone()],
                    format!(
                        "Walls {} and {} overlap along their length",
                        first.id().as_str(),
                        second.id().as_str()
                    ),
                ));
            }
        }
    }
    found
}

/// A door or window whose opening takes a stretch of its host wall that a wall joined to the host
/// stands across ([`crosses_stretch`]): the opening would have to cut the joined wall. Its
/// elements, in this order: the insert, its host wall, the joined wall. An insert whose host is
/// not a wall of the model is not judged.
fn crossing_joined(rule: &Rule, model: &Model) -> Vec<Warning> {
    let mut found = Vec::new();
    for insert in &model.inserts {
        let Some(host) = model.walls.iter().find(|w| *w.id() == insert.host) else {
            continue;
        };
        for other in &model.walls {
            if crosses_stretch(host, other, insert.axis, insert.low, insert.high) {
                found.push(rule.warn(
                    vec![insert.id.clone(), host.id().clone(), other.id().clone()],
                    format!(
                        "{} crosses wall {}, which joins its host wall {}: its opening would \
                         have to cut the joined wall",
                        capital(&insert.who),
                        other.id().as_str(),
                        host.id().as_str()
                    ),
                ));
            }
        }
    }
    found
}

fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The warnings a model holds now: every kind's ([`RULES`]), sorted by kind name then by
/// elements, each identity once.
#[must_use]
pub fn warnings(model: &Model) -> Vec<Warning> {
    let mut all: Vec<Warning> = RULES
        .iter()
        .flat_map(|rule| (rule.find)(rule, model))
        .collect();
    all.sort_by(|a, b| (a.kind, &a.elements).cmp(&(b.kind, &b.elements)));
    all.dedup_by(|a, b| a.kind == b.kind && a.elements == b.elements);
    all
}
