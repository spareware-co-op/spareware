//! A document's schedules by ID, with the rule a command checks before it writes one: a name
//! unique in the document without regard to case. Reading a document's records never applies it,
//! so an old document always reads: it binds what is written from now on.

use crate::error::ScheduleError;
use crate::schedule::{SCHEDULE_KIND, Schedule};
use construct_model::element::Element;
use kit_core::Id;
use std::collections::BTreeMap;

/// A document's schedules.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Schedules {
    by_id: BTreeMap<Id, Schedule>,
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl Schedules {
    /// No schedules.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A document's schedules from its records: every record of kind schedule. The same records
    /// in any order give the same schedules. Two records of one name are both read: the name
    /// rule binds commands, not reading.
    ///
    /// # Errors
    /// [`ScheduleError::Malformed`] for a schedule record that is not as written.
    pub fn from_records<'a>(
        records: impl IntoIterator<Item = &'a Element>,
    ) -> Result<Self, ScheduleError> {
        let mut schedules = Self::new();
        for record in records
            .into_iter()
            .filter(|record| record.kind == SCHEDULE_KIND)
        {
            let schedule = Schedule::from_record(record)?;
            schedules.by_id.insert(schedule.id().clone(), schedule);
        }
        Ok(schedules)
    }

    /// The schedule of this ID.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&Schedule> {
        self.by_id.get(id)
    }

    /// How many schedules there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Every schedule, by name without regard to case, then by ID.
    #[must_use]
    pub fn ordered(&self) -> Vec<&Schedule> {
        let mut all: Vec<&Schedule> = self.by_id.values().collect();
        all.sort_by(|a, b| {
            a.name()
                .to_lowercase()
                .cmp(&b.name().to_lowercase())
                .then_with(|| a.id().cmp(b.id()))
        });
        all
    }

    /// Every schedule as a record of `document`, in ID order.
    #[must_use]
    pub fn to_records(&self, document: &Id) -> Vec<Element> {
        self.by_id
            .values()
            .map(|schedule| schedule.to_record(document))
            .collect()
    }

    /// Refuses a name another schedule has, case aside.
    fn name_free(&self, name: &str, except: &Id) -> Result<(), ScheduleError> {
        if self
            .by_id
            .values()
            .any(|schedule| schedule.id() != except && same_name(schedule.name(), name))
        {
            return Err(ScheduleError::DuplicateName(name.to_owned()));
        }
        Ok(())
    }

    /// Adds a schedule.
    ///
    /// # Errors
    /// [`ScheduleError::DuplicateSchedule`] for an ID a schedule has,
    /// [`ScheduleError::DuplicateName`] for a name one has.
    pub fn add(&mut self, schedule: Schedule) -> Result<(), ScheduleError> {
        if self.by_id.contains_key(schedule.id()) {
            return Err(ScheduleError::DuplicateSchedule(schedule.id().clone()));
        }
        self.name_free(schedule.name(), schedule.id())?;
        self.by_id.insert(schedule.id().clone(), schedule);
        Ok(())
    }

    /// Puts a changed schedule in the place of the one of its ID. A schedule may change the case
    /// of its own name.
    ///
    /// # Errors
    /// [`ScheduleError::UnknownSchedule`] if no schedule has its ID,
    /// [`ScheduleError::DuplicateName`] for a name another schedule has.
    pub fn replace(&mut self, schedule: Schedule) -> Result<(), ScheduleError> {
        if !self.by_id.contains_key(schedule.id()) {
            return Err(ScheduleError::UnknownSchedule(schedule.id().clone()));
        }
        self.name_free(schedule.name(), schedule.id())?;
        self.by_id.insert(schedule.id().clone(), schedule);
        Ok(())
    }

    /// Takes a schedule away and gives it back.
    ///
    /// # Errors
    /// [`ScheduleError::UnknownSchedule`] if no schedule has the ID.
    pub fn remove(&mut self, id: &Id) -> Result<Schedule, ScheduleError> {
        self.by_id
            .remove(id)
            .ok_or_else(|| ScheduleError::UnknownSchedule(id.clone()))
    }
}
