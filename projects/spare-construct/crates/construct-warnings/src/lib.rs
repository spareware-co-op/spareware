//! Spare Construct: warnings as data, the problems a model holds (CON-A2-F7-P1).
//!
//! A warning is a plain value: its kind (a stable name), how serious it is, the elements involved
//! (their permanent IDs, in a stated order) and a sentence for people. People, AI and scripts read
//! the same list. Map: projects/spare-construct/ARCHITECTURE.md; the decisions are in
//! notes/CON-A2-F7-P1-warnings.md and the read-only form in FORMAT.md beside this crate.
//!
//! - [`warnings`]: the one function. It takes a [`Model`], the elements of a document as they
//!   stand (its walls as [`Wall`]s, its rooms as [`Room`]s), and gives the warnings now, in a
//!   stated order. Nothing is stored: the list is worked out each time it is asked, so after any
//!   edit it is the new list and a warning can never be out of date. A warning's identity
//!   ([`Warning::identity`]) is its kind plus its elements, so the same problem has the same
//!   identity across edits that do not touch it.
//! - [`RULES`]: the kinds, one entry each ([`Rule`]): its name, its [`Severity`] and the function
//!   that finds its warnings. A new kind is one more entry in the table and one function.
//! - [`wall`]: [`Wall`], an outline with a direction on a level, and [`overlap_along_length`],
//!   the exact test of two of them.
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

pub use wall::{LIMIT, MAX_CORNERS, Wall, WallError, overlap_along_length};

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

/// The elements of a document the warnings read, as they stand now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Model {
    /// Every wall that builds, with its level.
    pub walls: Vec<Wall>,
    /// Every room, with what became of it.
    pub rooms: Vec<Room>,
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
pub const RULES: [Rule; 2] = [
    Rule {
        kind: "room.not_enclosed",
        severity: Severity::Warning,
        find: not_enclosed,
    },
    Rule {
        kind: "wall.overlap",
        severity: Severity::Warning,
        find: overlapping,
    },
];

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
