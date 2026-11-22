//! Design options: sets of alternatives for one part of the building.
//!
//! A project has option sets, such as "entrance". Each set has named options, such as "ramp" and
//! "steps", and exactly one primary option, the one that is part of the building unless a view
//! chooses otherwise. An element belongs to the main model or to exactly one option of one set.
//! An ID is never shared by two sets or options.

use crate::error::PhaseError;
use crate::phases::check_name;
use kit_core::Id;
use serde::{Deserialize, Serialize};

/// One named alternative within a set.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DesignOption {
    /// Its permanent ID.
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    /// The name people see.
    pub name: String,
}

/// A set of alternatives with one primary.
///
/// Stored as `{"id":...,"name":...,"options":[...],"primary":<id>}`, the options in the order they
/// were added.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct OptionSet {
    /// Its permanent ID.
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    /// The name people see.
    pub name: String,
    /// The options, in the order they were added. Never empty.
    pub options: Vec<DesignOption>,
    /// The primary option: one of `options`.
    #[serde(with = "kit_ops::id")]
    pub primary: Id,
}

/// The place an element holds in a design option: which set, and which option of it. An element
/// with none of these is in the main model.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct OptionRef {
    /// The option set.
    #[serde(with = "kit_ops::id")]
    pub set: Id,
    /// The option within it.
    #[serde(with = "kit_ops::id")]
    pub option: Id,
}

/// A project's option sets, in the order they were added. Stored as a JSON array of
/// [`OptionSet`]. Reading one checks it: no repeated ID or name, no empty set, every primary one
/// of its set's options.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(try_from = "Vec<OptionSet>", into = "Vec<OptionSet>")]
pub struct DesignOptions {
    sets: Vec<OptionSet>,
}

impl TryFrom<Vec<OptionSet>> for DesignOptions {
    type Error = PhaseError;

    fn try_from(sets: Vec<OptionSet>) -> Result<Self, PhaseError> {
        let mut result = Self::default();
        for set in sets {
            let mut options = set.options.iter();
            let first = options
                .next()
                .ok_or_else(|| PhaseError::EmptySet(set.id.clone()))?;
            result = result.add_set(set.id.clone(), &set.name, first.id.clone(), &first.name)?;
            for option in options {
                result = result.add_option(&set.id, option.id.clone(), &option.name)?;
            }
            result = result
                .make_primary(&set.id, &set.primary)
                .map_err(|error| match error {
                    PhaseError::UnknownOption { set, option } => PhaseError::PrimaryNotInSet {
                        set,
                        primary: option,
                    },
                    other => other,
                })?;
        }
        Ok(result)
    }
}

impl From<DesignOptions> for Vec<OptionSet> {
    fn from(options: DesignOptions) -> Self {
        options.sets
    }
}

impl DesignOptions {
    /// A project with no option sets.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The sets, in the order they were added.
    #[must_use]
    pub fn sets(&self) -> &[OptionSet] {
        &self.sets
    }

    /// The set with this ID.
    #[must_use]
    pub fn set(&self, id: &Id) -> Option<&OptionSet> {
        self.sets.iter().find(|set| set.id == *id)
    }

    fn id_used(&self, id: &Id) -> bool {
        self.sets
            .iter()
            .any(|set| set.id == *id || set.options.iter().any(|option| option.id == *id))
    }

    /// The project with a new set holding one option, which is its primary. A set always has a
    /// primary, so it is made with its first option. Refused if an ID is already used anywhere
    /// among sets and options, a name is empty, or the set's name is already used by another set.
    ///
    /// # Errors
    ///
    /// Returns the [`PhaseError`] that names the refusal described above.
    pub fn add_set(
        &self,
        set_id: Id,
        set_name: &str,
        option_id: Id,
        option_name: &str,
    ) -> Result<Self, PhaseError> {
        check_name(set_name)?;
        check_name(option_name)?;
        if self.id_used(&set_id) {
            return Err(PhaseError::DuplicateId(set_id));
        }
        if set_id == option_id || self.id_used(&option_id) {
            return Err(PhaseError::DuplicateId(option_id));
        }
        if self.sets.iter().any(|set| set.name == set_name) {
            return Err(PhaseError::DuplicateName(set_name.to_owned()));
        }
        let mut sets = self.sets.clone();
        sets.push(OptionSet {
            id: set_id,
            name: set_name.to_owned(),
            options: vec![DesignOption {
                id: option_id.clone(),
                name: option_name.to_owned(),
            }],
            primary: option_id,
        });
        Ok(Self { sets })
    }

    /// The project with a new option at the end of a set. The primary does not change. Refused if
    /// the set is unknown, the ID is used anywhere, or the name is empty or used in this set.
    ///
    /// # Errors
    ///
    /// Returns the [`PhaseError`] that names the refusal described above.
    pub fn add_option(&self, set_id: &Id, option_id: Id, name: &str) -> Result<Self, PhaseError> {
        check_name(name)?;
        let at = self
            .sets
            .iter()
            .position(|set| set.id == *set_id)
            .ok_or_else(|| PhaseError::UnknownSet(set_id.clone()))?;
        if self.id_used(&option_id) {
            return Err(PhaseError::DuplicateId(option_id));
        }
        let mut sets = self.sets.clone();
        if let Some(set) = sets.get_mut(at) {
            if set.options.iter().any(|option| option.name == name) {
                return Err(PhaseError::DuplicateName(name.to_owned()));
            }
            set.options.push(DesignOption {
                id: option_id,
                name: name.to_owned(),
            });
        }
        Ok(Self { sets })
    }

    /// The project with one option made the primary of its set. The old primary stays an option.
    ///
    /// # Errors
    ///
    /// Returns the [`PhaseError`] that names the refusal described above.
    pub fn make_primary(&self, set_id: &Id, option_id: &Id) -> Result<Self, PhaseError> {
        let at = self
            .sets
            .iter()
            .position(|set| set.id == *set_id)
            .ok_or_else(|| PhaseError::UnknownSet(set_id.clone()))?;
        let mut sets = self.sets.clone();
        if let Some(set) = sets.get_mut(at) {
            if !set.options.iter().any(|option| option.id == *option_id) {
                return Err(PhaseError::UnknownOption {
                    set: set_id.clone(),
                    option: option_id.clone(),
                });
            }
            set.primary = option_id.clone();
        }
        Ok(Self { sets })
    }

    /// Checks that a membership names a set and one of its options.
    ///
    /// # Errors
    ///
    /// Returns the [`PhaseError`] that names the refusal described above.
    pub fn check(&self, reference: &OptionRef) -> Result<(), PhaseError> {
        let set = self
            .set(&reference.set)
            .ok_or_else(|| PhaseError::UnknownSet(reference.set.clone()))?;
        if set
            .options
            .iter()
            .any(|option| option.id == reference.option)
        {
            Ok(())
        } else {
            Err(PhaseError::UnknownOption {
                set: reference.set.clone(),
                option: reference.option.clone(),
            })
        }
    }
}
