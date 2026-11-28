//! What one curtain wall changes of its type's grid, each by where it stands so that it means the
//! same whatever else changes: grid lines it adds, grid lines of its type it removes, segments of
//! grid lines it leaves out, and panels it swaps. A grid line is named by its direction and its
//! place (a vertical line by its distance along the wall from its start, a horizontal one by its
//! height above the base); a segment by its line and the crossing it starts at; a panel by the
//! corner its cell starts at (its lowest grid line in each direction).
//!
//! An edit the grid no longer has a place for (a removed line its type no longer lays out, a
//! segment of a line that is gone, a swap of a panel that has merged into another) is kept and
//! does nothing; the layout lists it as unused ([`crate::Grid::unused`]), so a change to the type
//! never fails a wall, and changing the type back brings the edit back.
//!
//! The stored form ([`Edits::to_json`], [`Edits::from_json`]): a JSON object in the recipe's `grid`
//! argument, every list in order and without repeats, every length in construct-params' stored
//! spelling; no edits is no argument (FORMAT.md beside this crate).

use crate::error::CurtainError;
use crate::grid::Direction;
use crate::length;
use crate::spec::Panel;
use crate::stored::{StoredPanel, panel_from_stored, panel_to_stored};
use construct_params::Length;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};

/// A segment of a grid line: the part between two neighbouring crossings, named by its line and
/// the crossing it starts at (the lower one of a vertical line, the one nearer the start of a
/// horizontal line; a border's own start, 0, for the first).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Segment {
    /// The line's direction.
    pub direction: Direction,
    /// Where the line stands.
    pub at: Length,
    /// The crossing the segment starts at.
    pub from: Length,
}

/// One curtain wall's own changes to its type's grid.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Edits {
    added: [BTreeSet<Length>; 2],
    removed: [BTreeSet<Length>; 2],
    omitted: BTreeSet<Segment>,
    panels: BTreeMap<[Length; 2], Panel>,
}

/// The slot of a direction in the two-slot lists.
fn slot(direction: Direction) -> usize {
    match direction {
        Direction::Vertical => 0,
        Direction::Horizontal => 1,
    }
}

impl Edits {
    /// No edits: the wall is laid out as its type says.
    #[must_use]
    pub fn new() -> Edits {
        Edits::default()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.iter().all(BTreeSet::is_empty)
            && self.removed.iter().all(BTreeSet::is_empty)
            && self.omitted.is_empty()
            && self.panels.is_empty()
    }

    /// The lines of `direction` it adds, in order.
    pub fn added(&self, direction: Direction) -> impl Iterator<Item = Length> + '_ {
        self.added
            .get(slot(direction))
            .into_iter()
            .flatten()
            .copied()
    }

    /// The lines of its type of `direction` it removes, in order.
    pub fn removed(&self, direction: Direction) -> impl Iterator<Item = Length> + '_ {
        self.removed
            .get(slot(direction))
            .into_iter()
            .flatten()
            .copied()
    }

    /// Whether it adds a line of `direction` at `at`.
    #[must_use]
    pub fn adds(&self, direction: Direction, at: Length) -> bool {
        self.added
            .get(slot(direction))
            .is_some_and(|set| set.contains(&at))
    }

    /// Whether it removes its type's line of `direction` at `at`.
    #[must_use]
    pub fn removes(&self, direction: Direction, at: Length) -> bool {
        self.removed
            .get(slot(direction))
            .is_some_and(|set| set.contains(&at))
    }

    /// The segments it leaves out, in order.
    pub fn omitted(&self) -> impl Iterator<Item = &Segment> {
        self.omitted.iter()
    }

    /// Whether it leaves `segment` out.
    #[must_use]
    pub fn omits(&self, segment: &Segment) -> bool {
        self.omitted.contains(segment)
    }

    /// The panels it swaps, by the corner their cell starts at, in order.
    pub fn panels(&self) -> impl Iterator<Item = (&[Length; 2], &Panel)> {
        self.panels.iter()
    }

    /// The panel it puts in the cell that starts at `at`, if it swaps one.
    #[must_use]
    pub fn panel_at(&self, at: &[Length; 2]) -> Option<&Panel> {
        self.panels.get(at)
    }

    /// Adds a line of `direction` at `at`; `false` if it had it already.
    pub fn add(&mut self, direction: Direction, at: Length) -> bool {
        self.added
            .get_mut(slot(direction))
            .is_some_and(|set| set.insert(at))
    }

    /// No longer adds that line; `false` if it did not.
    pub fn unadd(&mut self, direction: Direction, at: Length) -> bool {
        self.added
            .get_mut(slot(direction))
            .is_some_and(|set| set.remove(&at))
    }

    /// Removes its type's line of `direction` at `at`; `false` if it had removed it already.
    pub fn remove(&mut self, direction: Direction, at: Length) -> bool {
        self.removed
            .get_mut(slot(direction))
            .is_some_and(|set| set.insert(at))
    }

    /// No longer removes that line of its type; `false` if it did not.
    pub fn unremove(&mut self, direction: Direction, at: Length) -> bool {
        self.removed
            .get_mut(slot(direction))
            .is_some_and(|set| set.remove(&at))
    }

    /// Leaves `segment` out; `false` if it did already.
    pub fn omit(&mut self, segment: Segment) -> bool {
        self.omitted.insert(segment)
    }

    /// Keeps `segment` again; `false` if it was not left out.
    pub fn restore(&mut self, segment: &Segment) -> bool {
        self.omitted.remove(segment)
    }

    /// Forgets every left-out segment of the line of `direction` at `at`.
    pub fn restore_line(&mut self, direction: Direction, at: Length) {
        self.omitted
            .retain(|s| !(s.direction == direction && s.at == at));
    }

    /// Puts `panel` in the cell that starts at `at`, or the type's panel again for `None`.
    pub fn swap(&mut self, at: [Length; 2], panel: Option<Panel>) {
        match panel {
            Some(panel) => {
                self.panels.insert(at, panel);
            }
            None => {
                self.panels.remove(&at);
            }
        }
    }
}

/// The stored form, field for field.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    added: Lines,
    removed: Lines,
    omitted: Vec<StoredSegment>,
    panels: Vec<StoredSwap>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Lines {
    vertical: Vec<String>,
    horizontal: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredSegment {
    line: Direction,
    at: String,
    from: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredSwap {
    at: [String; 2],
    panel: StoredPanel,
}

/// A stored length, or the refusal naming what it is.
fn read_length(text: &str, what: &str) -> Result<Length, CurtainError> {
    length::read(text)
        .ok_or_else(|| CurtainError::Malformed(format!("{what} {text:?} is not a stored length")))
}

/// Lengths read from a list that must be in order without repeats.
fn read_lines(list: &[String], what: &str) -> Result<BTreeSet<Length>, CurtainError> {
    let mut out = BTreeSet::new();
    let mut previous: Option<Length> = None;
    for text in list {
        let at = read_length(text, what)?;
        if previous.is_some_and(|before| before >= at) {
            return Err(CurtainError::Malformed(format!(
                "the {what} lines are not in order without repeats"
            )));
        }
        previous = Some(at);
        out.insert(at);
    }
    Ok(out)
}

impl Edits {
    /// The stored form: `None` when there are no edits, which is no argument at all.
    ///
    /// # Errors
    /// [`CurtainError::Malformed`] only if the JSON writer fails, which it does not for this data.
    pub fn to_json(&self) -> Result<Option<Json>, CurtainError> {
        if self.is_empty() {
            return Ok(None);
        }
        let lines = |sets: &[BTreeSet<Length>; 2]| Lines {
            vertical: sets
                .first()
                .into_iter()
                .flatten()
                .map(|l| length::write(*l))
                .collect(),
            horizontal: sets
                .get(1)
                .into_iter()
                .flatten()
                .map(|l| length::write(*l))
                .collect(),
        };
        let stored = Stored {
            added: lines(&self.added),
            removed: lines(&self.removed),
            omitted: self
                .omitted
                .iter()
                .map(|s| StoredSegment {
                    line: s.direction,
                    at: length::write(s.at),
                    from: length::write(s.from),
                })
                .collect(),
            panels: self
                .panels
                .iter()
                .map(|([along, up], panel)| StoredSwap {
                    at: [length::write(*along), length::write(*up)],
                    panel: panel_to_stored(panel),
                })
                .collect(),
        };
        serde_json::to_value(&stored)
            .map(Some)
            .map_err(|e| CurtainError::Malformed(e.to_string()))
    }

    /// Reads what [`Edits::to_json`] writes; `None` is no edits. Anything else is refused: a field
    /// missing or added, a length not in its stored spelling, a list out of order or with a
    /// repeat, a panel that does not read, or an empty object (no edits is no argument).
    ///
    /// # Errors
    /// [`CurtainError::Malformed`], or a panel's refusal.
    pub fn from_json(json: Option<&Json>) -> Result<Edits, CurtainError> {
        let Some(json) = json else {
            return Ok(Edits::new());
        };
        let stored: Stored = serde_json::from_value(json.clone())
            .map_err(|e| CurtainError::Malformed(format!("grid: {e}")))?;
        let mut edits = Edits::new();
        edits.added = [
            read_lines(&stored.added.vertical, "added vertical")?,
            read_lines(&stored.added.horizontal, "added horizontal")?,
        ];
        edits.removed = [
            read_lines(&stored.removed.vertical, "removed vertical")?,
            read_lines(&stored.removed.horizontal, "removed horizontal")?,
        ];
        let mut last: Option<Segment> = None;
        for s in &stored.omitted {
            let segment = Segment {
                direction: s.line,
                at: read_length(&s.at, "a left-out segment's line")?,
                from: read_length(&s.from, "a left-out segment's start")?,
            };
            if last.is_some_and(|before| before >= segment) {
                return Err(CurtainError::Malformed(
                    "the left-out segments are not in order without repeats".to_owned(),
                ));
            }
            last = Some(segment);
            edits.omitted.insert(segment);
        }
        let mut last: Option<[Length; 2]> = None;
        for swap in &stored.panels {
            let [along, up] = &swap.at;
            let at = [
                read_length(along, "a swapped panel's corner")?,
                read_length(up, "a swapped panel's corner")?,
            ];
            if last.is_some_and(|before| before >= at) {
                return Err(CurtainError::Malformed(
                    "the swapped panels are not in order without repeats".to_owned(),
                ));
            }
            last = Some(at);
            edits.panels.insert(at, panel_from_stored(&swap.panel)?);
        }
        if edits.is_empty() {
            return Err(CurtainError::Malformed(
                "a grid with no edits is left out, not written empty".to_owned(),
            ));
        }
        if edits.to_json()?.as_ref() != Some(json) {
            return Err(CurtainError::Malformed(
                "the grid is not in its one spelling".to_owned(),
            ));
        }
        Ok(edits)
    }
}
