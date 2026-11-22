//! A document's levels by ID, with the rules a command checks before it writes one: a name unique
//! in the document without regard to case, and at most one storey at any elevation. Reading a
//! document's records never applies these rules, so an old document always reads: they bind what
//! is written from now on.

use crate::error::LevelError;
use crate::implied;
use crate::implied::implied_number;
use crate::level::{LEVEL_KIND, Level, clean_name};
use crate::tie::{base_of, top_of};
use construct_model::element::Element;
use construct_params::Length;
use kit_core::Id;
use std::collections::BTreeMap;

/// Whether two level names are the same name, case aside.
fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// A level and whether it is implied: named by an old integer, with no record of its own.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    level: Level,
    implied: bool,
}

/// A document's levels: those with a record, and those its old records' integers imply.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Levels {
    by_id: BTreeMap<Id, Entry>,
}

impl Levels {
    /// No levels.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A document's levels from its records: every level record, and the implied level of every
    /// ID an element's tie or old integer names that no record holds. The same records in any
    /// order give the same levels. A tie naming an ID that is neither is left out: it is a
    /// failure of that element's shape, not of the document.
    ///
    /// # Errors
    /// [`LevelError::Malformed`] for a level record or a tie that is not as written.
    pub fn from_records<'a>(
        records: impl IntoIterator<Item = &'a Element>,
    ) -> Result<Self, LevelError> {
        let records: Vec<&Element> = records.into_iter().collect();
        let mut levels = Self::new();
        for record in records.iter().filter(|record| record.kind == LEVEL_KIND) {
            let level = Level::from_record(record)?;
            levels.by_id.insert(
                level.id().clone(),
                Entry {
                    level,
                    implied: false,
                },
            );
        }
        for record in records.iter().filter(|record| record.kind != LEVEL_KIND) {
            for tie in [base_of(record)?, top_of(record)?].into_iter().flatten() {
                if levels.by_id.contains_key(&tie.level) {
                    continue;
                }
                if let Some(level) = implied_number(&tie.level).and_then(implied::implied) {
                    levels.by_id.insert(
                        tie.level,
                        Entry {
                            level,
                            implied: true,
                        },
                    );
                }
            }
        }
        Ok(levels)
    }

    /// The level of this ID, stored or implied.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&Level> {
        self.by_id.get(id).map(|entry| &entry.level)
    }

    /// The level a tie to this ID stands on: a level of the document, or else the implied level
    /// of an ID in the implied form, in use or not.
    #[must_use]
    pub fn resolve(&self, id: &Id) -> Option<Level> {
        self.get(id)
            .cloned()
            .or_else(|| implied_number(id).and_then(implied::implied))
    }

    /// Whether the level of this ID is implied: it has no record yet.
    #[must_use]
    pub fn is_implied(&self, id: &Id) -> bool {
        self.by_id.get(id).is_some_and(|entry| entry.implied)
    }

    /// How many levels there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Every level, lowest first: by elevation, then by name without regard to case, then by ID.
    #[must_use]
    pub fn ordered(&self) -> Vec<&Level> {
        let mut all: Vec<&Level> = self.by_id.values().map(|entry| &entry.level).collect();
        all.sort_by(|a, b| {
            a.elevation()
                .cmp(&b.elevation())
                .then_with(|| a.name().to_lowercase().cmp(&b.name().to_lowercase()))
                .then_with(|| a.id().cmp(b.id()))
        });
        all
    }

    /// Refuses a name another level has, case aside.
    fn name_free(&self, name: &str, except: &Id) -> Result<(), LevelError> {
        if self
            .by_id
            .values()
            .any(|entry| entry.level.id() != except && same_name(entry.level.name(), name))
        {
            return Err(LevelError::DuplicateName(name.to_owned()));
        }
        Ok(())
    }

    /// Refuses a storey at an elevation where another level is already the storey.
    fn storey_free(&self, level: &Level) -> Result<(), LevelError> {
        if !level.is_storey() {
            return Ok(());
        }
        let other = self.by_id.values().find(|entry| {
            entry.level.id() != level.id()
                && entry.level.is_storey()
                && entry.level.elevation() == level.elevation()
        });
        match other {
            Some(other) => Err(LevelError::StoreyAtElevation {
                name: level.name().to_owned(),
                other: other.level.name().to_owned(),
                elevation: level.elevation(),
            }),
            None => Ok(()),
        }
    }

    /// Adds a level, as a stored one.
    ///
    /// # Errors
    /// [`LevelError::DuplicateLevel`] for an ID a level has, [`LevelError::DuplicateName`] for a
    /// name one has, [`LevelError::StoreyAtElevation`] for a second storey at one elevation.
    pub fn add(&mut self, level: Level) -> Result<(), LevelError> {
        if self.by_id.contains_key(level.id()) {
            return Err(LevelError::DuplicateLevel(level.id().clone()));
        }
        self.name_free(level.name(), level.id())?;
        self.storey_free(&level)?;
        self.by_id.insert(
            level.id().clone(),
            Entry {
                level,
                implied: false,
            },
        );
        Ok(())
    }

    /// Changes one level, checks the rules, and keeps it as a stored level. Nothing changes on a
    /// refusal.
    fn change(&mut self, id: &Id, edit: impl FnOnce(&mut Level)) -> Result<&Level, LevelError> {
        let mut level = self
            .get(id)
            .cloned()
            .ok_or_else(|| LevelError::UnknownLevel(id.clone()))?;
        edit(&mut level);
        self.name_free(level.name(), id)?;
        self.storey_free(&level)?;
        let entry = self.by_id.entry(id.clone()).or_insert(Entry {
            level: level.clone(),
            implied: false,
        });
        entry.level = level;
        entry.implied = false;
        Ok(&entry.level)
    }

    /// Renames a level; the name is trimmed, and a level may change the case of its own name.
    ///
    /// # Errors
    /// [`LevelError::UnknownLevel`], [`LevelError::EmptyName`], [`LevelError::DuplicateName`].
    pub fn rename(&mut self, id: &Id, name: &str) -> Result<&Level, LevelError> {
        let name = clean_name(name)?;
        self.change(id, |level| level.set_name(name))
    }

    /// Moves a level to another elevation.
    ///
    /// # Errors
    /// [`LevelError::UnknownLevel`], [`LevelError::StoreyAtElevation`].
    pub fn set_elevation(&mut self, id: &Id, elevation: Length) -> Result<&Level, LevelError> {
        self.change(id, |level| level.set_elevation(elevation))
    }

    /// Makes a level a storey, or a reference datum.
    ///
    /// # Errors
    /// [`LevelError::UnknownLevel`], [`LevelError::StoreyAtElevation`].
    pub fn set_storey(&mut self, id: &Id, storey: bool) -> Result<&Level, LevelError> {
        self.change(id, |level| level.set_storey(storey))
    }

    /// Takes a level away, given how many elements are tied to it ([`tied_to`]).
    ///
    /// # Errors
    /// [`LevelError::UnknownLevel`]; [`LevelError::Tied`] naming how many are tied to it.
    pub fn remove(&mut self, id: &Id, tied: usize) -> Result<Level, LevelError> {
        let level = self
            .get(id)
            .ok_or_else(|| LevelError::UnknownLevel(id.clone()))?;
        if tied > 0 {
            return Err(LevelError::Tied {
                name: level.name().to_owned(),
                count: tied,
            });
        }
        self.by_id
            .remove(id)
            .map(|entry| entry.level)
            .ok_or_else(|| LevelError::UnknownLevel(id.clone()))
    }
}

/// How many of `records` are tied to the level `id`, by the base or the top: an element tied by
/// both counts once.
///
/// # Errors
/// [`LevelError::Malformed`] for a tie that is not as written.
pub fn tied_to<'a>(
    id: &Id,
    records: impl IntoIterator<Item = &'a Element>,
) -> Result<usize, LevelError> {
    let mut count: usize = 0;
    for record in records {
        if record.kind == LEVEL_KIND {
            continue;
        }
        let base = base_of(record)?.is_some_and(|tie| tie.level == *id);
        let top = top_of(record)?.is_some_and(|tie| tie.level == *id);
        if base || top {
            count = count.saturating_add(1);
        }
    }
    Ok(count)
}
