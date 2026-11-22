//! A project's ordered phases, and the staging fields every element carries.
//!
//! The list runs from the earliest stage to the latest, such as existing then new works. A phase's
//! position is its place in that list; its ID never changes, so renaming or moving a phase leaves
//! every element's fields pointing at the same phase.

use crate::error::PhaseError;
use crate::options::{DesignOptions, OptionRef};
use kit_core::Id;
use serde::{Deserialize, Serialize};

/// One stage of the work.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Phase {
    /// Its permanent ID.
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    /// The name people see.
    pub name: String,
}

/// A project's phases, earliest first. Stored as a JSON array in that order, each phase as
/// `{"id":...,"name":...}`. Reading one checks it, so a list can't hold a repeated ID or name.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(try_from = "Vec<Phase>", into = "Vec<Phase>")]
pub struct PhaseList {
    phases: Vec<Phase>,
}

impl TryFrom<Vec<Phase>> for PhaseList {
    type Error = PhaseError;

    fn try_from(phases: Vec<Phase>) -> Result<Self, PhaseError> {
        let mut list = Self::default();
        for phase in phases {
            list = list.add(phase.id, &phase.name)?;
        }
        Ok(list)
    }
}

impl From<PhaseList> for Vec<Phase> {
    fn from(list: PhaseList) -> Self {
        list.phases
    }
}

pub(crate) fn check_name(name: &str) -> Result<(), PhaseError> {
    if name.trim().is_empty() {
        Err(PhaseError::EmptyName)
    } else {
        Ok(())
    }
}

impl PhaseList {
    /// A list with no phases.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The phases, earliest first.
    #[must_use]
    pub fn phases(&self) -> &[Phase] {
        &self.phases
    }

    /// Where the phase sits, counting from 0 (earliest), or `None` if there is no such phase.
    #[must_use]
    pub fn position(&self, id: &Id) -> Option<usize> {
        self.phases.iter().position(|phase| phase.id == *id)
    }

    /// The list with a new phase after the last one. Refused if the name is empty or already used,
    /// or the ID is already used.
    ///
    /// # Errors
    ///
    /// Returns the [`PhaseError`] that names the refusal described above.
    pub fn add(&self, id: Id, name: &str) -> Result<Self, PhaseError> {
        check_name(name)?;
        if self.position(&id).is_some() {
            return Err(PhaseError::DuplicateId(id));
        }
        if self.phases.iter().any(|phase| phase.name == name) {
            return Err(PhaseError::DuplicateName(name.to_owned()));
        }
        let mut phases = self.phases.clone();
        phases.push(Phase {
            id,
            name: name.to_owned(),
        });
        Ok(Self { phases })
    }

    /// The list with one phase under a new name. Its ID and place stay. Renaming a phase to the
    /// name it already has is allowed.
    ///
    /// # Errors
    ///
    /// Returns the [`PhaseError`] that names the refusal described above.
    pub fn rename(&self, id: &Id, name: &str) -> Result<Self, PhaseError> {
        check_name(name)?;
        let at = self
            .position(id)
            .ok_or_else(|| PhaseError::UnknownPhase(id.clone()))?;
        if self
            .phases
            .iter()
            .enumerate()
            .any(|(index, phase)| index != at && phase.name == name)
        {
            return Err(PhaseError::DuplicateName(name.to_owned()));
        }
        let mut phases = self.phases.clone();
        if let Some(phase) = phases.get_mut(at) {
            name.clone_into(&mut phase.name);
        }
        Ok(Self { phases })
    }

    /// The list with one phase moved so that it sits at `position` (counting from 0), the others
    /// keeping their order. Moving it where it already is changes nothing.
    ///
    /// Moving phases can put a stored demolished phase before its created phase; call
    /// [`ElementStaging::check`] again for elements after a move.
    ///
    /// # Errors
    ///
    /// Returns the [`PhaseError`] that names the refusal described above.
    pub fn move_to(&self, id: &Id, position: usize) -> Result<Self, PhaseError> {
        let from = self
            .position(id)
            .ok_or_else(|| PhaseError::UnknownPhase(id.clone()))?;
        if position >= self.phases.len() {
            return Err(PhaseError::PositionOutOfRange {
                position,
                len: self.phases.len(),
            });
        }
        let mut phases = self.phases.clone();
        let phase = phases.remove(from);
        phases.insert(position, phase);
        Ok(Self { phases })
    }
}

/// The staging fields every element carries: the phase it was created in, the phase it was
/// demolished in if any, and the design option it belongs to if any.
///
/// Stored as `{"created":<id>,"demolished":<id or null>,"design_option":<null or {"set":<id>,
/// "option":<id>}>}`. An element with no design option belongs to the main model. A demolished
/// phase equal to the created phase is allowed: the element is temporary in that phase.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ElementStaging {
    /// The phase the element was created in.
    #[serde(with = "kit_ops::id")]
    pub created: Id,
    /// The phase it was demolished in, never before `created`.
    #[serde(with = "kit_ops::id::option")]
    pub demolished: Option<Id>,
    /// The one option it belongs to, or `None` for the main model.
    pub design_option: Option<OptionRef>,
}

impl ElementStaging {
    /// Fields for an element, refused if a phase is unknown, the demolished phase comes before the
    /// created phase, or the option is unknown.
    ///
    /// # Errors
    ///
    /// Returns the [`PhaseError`] that names the refusal described above.
    pub fn new(
        phases: &PhaseList,
        options: &DesignOptions,
        created: Id,
        demolished: Option<Id>,
        design_option: Option<OptionRef>,
    ) -> Result<Self, PhaseError> {
        let staging = Self {
            created,
            demolished,
            design_option,
        };
        staging.check(phases, options)?;
        Ok(staging)
    }

    /// Checks the fields against the project's current phases and options.
    ///
    /// # Errors
    ///
    /// Returns the [`PhaseError`] that names the refusal described above.
    pub fn check(&self, phases: &PhaseList, options: &DesignOptions) -> Result<(), PhaseError> {
        let created = phases
            .position(&self.created)
            .ok_or_else(|| PhaseError::UnknownPhase(self.created.clone()))?;
        if let Some(demolished_id) = &self.demolished {
            let demolished = phases
                .position(demolished_id)
                .ok_or_else(|| PhaseError::UnknownPhase(demolished_id.clone()))?;
            if demolished < created {
                return Err(PhaseError::DemolishedBeforeCreated {
                    created: self.created.clone(),
                    demolished: demolished_id.clone(),
                });
            }
        }
        if let Some(reference) = &self.design_option {
            options.check(reference)?;
        }
        Ok(())
    }
}
