//! A schedule placed on a sheet: its own element, with a position on the paper.
//!
//! The placement says which schedule stands on which sheet and where its top left corner is,
//! measured from the paper's lower left corner, exact. It holds no rows and no drawing: what is
//! drawn is the schedule's live table every time it is asked for ([`crate::draw`]), so a sheet
//! never shows a schedule as it was. A schedule may stand on several sheets but on one sheet only
//! once. The placement is a record of kind [`PLACEMENT_KIND`] in the schedules category, with no
//! shape, read back strictly.

use crate::error::ScheduleError;
use construct_categories::SCHEDULES;
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Length, Value};
use kit_core::Id;
use std::collections::BTreeMap;

/// The `kind` of every placement record, and its recipe's operation.
pub const PLACEMENT_KIND: &str = "schedule-placement";
/// The record's category, as every record carries it (construct-types' key).
pub const CATEGORY_KEY: &str = "@category";
/// The schedule placed, an ID.
pub const SCHEDULE_KEY: &str = "@placement.schedule";
/// The sheet it stands on, an ID.
pub const SHEET_KEY: &str = "@placement.sheet";
/// The top left corner's distance across the paper from its left edge, a stored length.
pub const X_KEY: &str = "@placement.x";
/// The top left corner's distance up the paper from its bottom edge, a stored length.
pub const Y_KEY: &str = "@placement.y";
/// The name every placement record carries.
pub const PLACEMENT_NAME: &str = "Schedule on sheet";

const NO_PLACEMENT: Placement = Placement { x: 0, y: 0, z: 0 };

/// The recipe of a placement record: Spare Construct's own operation `schedule-placement`,
/// version 1, with no arguments. It builds no shape.
#[must_use]
pub fn placement_recipe() -> Recipe {
    Recipe::construct(PLACEMENT_KIND, BTreeMap::new())
}

fn stored(length: Length) -> String {
    Value::Length(length).stored()
}

fn length(text: &str) -> Option<Length> {
    match Value::from_stored(text)? {
        Value::Length(length) => Some(length),
        _ => None,
    }
}

/// A schedule placed on a sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchedulePlacement {
    id: Id,
    schedule: Id,
    sheet: Id,
    x: Length,
    y: Length,
}

impl SchedulePlacement {
    /// A placement with its top left corner at (`x`, `y`) on the paper.
    #[must_use]
    pub fn new(id: Id, schedule: Id, sheet: Id, x: Length, y: Length) -> Self {
        Self {
            id,
            schedule,
            sheet,
            x,
            y,
        }
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The schedule placed.
    #[must_use]
    pub fn schedule(&self) -> &Id {
        &self.schedule
    }

    /// The sheet it stands on.
    #[must_use]
    pub fn sheet(&self) -> &Id {
        &self.sheet
    }

    /// The top left corner's distance from the paper's left edge.
    #[must_use]
    pub fn x(&self) -> Length {
        self.x
    }

    /// The top left corner's distance from the paper's bottom edge.
    #[must_use]
    pub fn y(&self) -> Length {
        self.y
    }

    /// The same placement with its corner moved.
    #[must_use]
    pub fn moved(mut self, x: Length, y: Length) -> Self {
        self.x = x;
        self.y = y;
        self
    }

    /// The placement as its element record in `document`.
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = BTreeMap::new();
        props.insert(CATEGORY_KEY.to_owned(), SCHEDULES.0.to_string());
        props.insert(SCHEDULE_KEY.to_owned(), self.schedule.to_string());
        props.insert(SHEET_KEY.to_owned(), self.sheet.to_string());
        props.insert(X_KEY.to_owned(), stored(self.x));
        props.insert(Y_KEY.to_owned(), stored(self.y));
        Element {
            id: self.id.clone(),
            document: document.clone(),
            kind: PLACEMENT_KIND.to_owned(),
            name: PLACEMENT_NAME.to_owned(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: placement_recipe(),
            props,
            references: Vec::new(),
        }
    }

    /// Reads a placement from its element record. Anything [`SchedulePlacement::to_record`] would
    /// not write is refused, never repaired.
    ///
    /// # Errors
    /// [`ScheduleError::Malformed`], naming the record.
    pub fn from_record(record: &Element) -> Result<Self, ScheduleError> {
        let malformed = |why: &str| {
            ScheduleError::Malformed(format!("The placement record {} {why}.", record.id))
        };
        if record.kind != PLACEMENT_KIND || record.recipe != placement_recipe() {
            return Err(malformed(
                "is not of kind schedule-placement, with its recipe",
            ));
        }
        let id_of = |key: &str| record.props.get(key).and_then(|text| Id::parse(text).ok());
        let (Some(schedule), Some(sheet)) = (id_of(SCHEDULE_KEY), id_of(SHEET_KEY)) else {
            return Err(malformed("names no schedule or no sheet"));
        };
        let place = |key: &str| record.props.get(key).and_then(|text| length(text));
        let (Some(x), Some(y)) = (place(X_KEY), place(Y_KEY)) else {
            return Err(malformed("has no position"));
        };
        let placement = Self::new(record.id.clone(), schedule, sheet, x, y);
        if placement.to_record(&record.document) != *record {
            return Err(malformed("is not as a placement record is written"));
        }
        Ok(placement)
    }
}

/// A document's schedule placements by ID.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Placements {
    by_id: BTreeMap<Id, SchedulePlacement>,
}

impl Placements {
    /// No placements.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A document's placements from its records: every record of kind schedule-placement.
    ///
    /// # Errors
    /// [`ScheduleError::Malformed`] for a placement record that is not as written.
    pub fn from_records<'a>(
        records: impl IntoIterator<Item = &'a Element>,
    ) -> Result<Self, ScheduleError> {
        let mut all = Self::new();
        for record in records
            .into_iter()
            .filter(|record| record.kind == PLACEMENT_KIND)
        {
            let one = SchedulePlacement::from_record(record)?;
            all.by_id.insert(one.id().clone(), one);
        }
        Ok(all)
    }

    /// The placement of this ID.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&SchedulePlacement> {
        self.by_id.get(id)
    }

    /// Every placement, by ID.
    #[must_use]
    pub fn all(&self) -> Vec<&SchedulePlacement> {
        self.by_id.values().collect()
    }

    /// The placements on a sheet, by ID.
    #[must_use]
    pub fn of_sheet(&self, sheet: &Id) -> Vec<&SchedulePlacement> {
        self.by_id
            .values()
            .filter(|one| one.sheet() == sheet)
            .collect()
    }

    /// The placements of a schedule, by ID.
    #[must_use]
    pub fn of_schedule(&self, schedule: &Id) -> Vec<&SchedulePlacement> {
        self.by_id
            .values()
            .filter(|one| one.schedule() == schedule)
            .collect()
    }

    /// How many there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Adds a placement.
    ///
    /// # Errors
    /// [`ScheduleError::DuplicatePlacement`] for an ID one has, or for a schedule already on this
    /// sheet.
    pub fn add(&mut self, placement: SchedulePlacement) -> Result<(), ScheduleError> {
        if self.by_id.contains_key(placement.id())
            || self
                .of_sheet(placement.sheet())
                .iter()
                .any(|one| one.schedule() == placement.schedule())
        {
            return Err(ScheduleError::DuplicatePlacement(
                placement.schedule().clone(),
                placement.sheet().clone(),
            ));
        }
        self.by_id.insert(placement.id().clone(), placement);
        Ok(())
    }

    /// Puts a moved placement in the place of the one of its ID.
    ///
    /// # Errors
    /// [`ScheduleError::UnknownPlacement`] if none has its ID.
    pub fn replace(&mut self, placement: SchedulePlacement) -> Result<(), ScheduleError> {
        if !self.by_id.contains_key(placement.id()) {
            return Err(ScheduleError::UnknownPlacement(placement.id().clone()));
        }
        self.by_id.insert(placement.id().clone(), placement);
        Ok(())
    }

    /// Takes a placement away and gives it back.
    ///
    /// # Errors
    /// [`ScheduleError::UnknownPlacement`].
    pub fn remove(&mut self, id: &Id) -> Result<SchedulePlacement, ScheduleError> {
        self.by_id
            .remove(id)
            .ok_or_else(|| ScheduleError::UnknownPlacement(id.clone()))
    }
}
