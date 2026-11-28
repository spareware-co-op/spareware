//! A schedule's rows: a live query, worked out from the elements as they are, never stored.
//!
//! [`rows`] takes a schedule and what is known of each element ([`Subject`]) and gives one [`Row`]
//! for each element the schedule lists: an element of its category (or a subcategory of it) for
//! which every filter holds, in the order of its sorting. The same function over the elements
//! after an edit gives the new rows, so a row appears when an element is added or comes to pass a
//! filter, changes when its values do, and leaves when it is deleted or stops passing.
//!
//! The order: by each sort key in turn, by exact value for numbers ([`crate::order`]) and by
//! [`crate::text_order`] for text; a row with no value for a key comes after every row that has
//! one, in either direction; rows left level are in element ID order, so the order is total and
//! the same on every run.
//!
//! A field the category no longer has (a parameter taken off it since the schedule was made)
//! gives an empty cell, and a filter on it lists nothing: [`crate::Schedule::check`] is what says
//! so in words, and a reader of rows is not stopped by it.

use crate::calc;
use crate::field::{Basis, Field};
use crate::filter::order;
use crate::schedule::Schedule;
use construct_categories::CategoryId;
use construct_params::{Kind, Units, Value};
use kit_core::Id;
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// A value an element has for a field, and how it is shown in the document's units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading {
    /// The exact value.
    pub value: Value,
    /// The value with its unit, as the document shows it (`12.50 m²`).
    pub shown: String,
}

/// What is known of one element: the facts a schedule reads, got by the caller from the
/// element's record, its type and the parameter table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subject {
    /// The element's ID.
    pub id: Id,
    /// The category it is in.
    pub category: CategoryId,
    /// Its name.
    pub name: String,
    /// The name of its type, if it has one.
    pub type_name: Option<String>,
    /// Its parameter values and read-only quantities by [`Field::key`]. A field with no entry
    /// has no value for this element.
    pub readings: BTreeMap<String, Reading>,
}

impl Subject {
    /// What the element has for a field.
    #[must_use]
    pub fn reading(&self, field: &Field) -> Option<Reading> {
        match field {
            Field::Name => Some(Reading {
                value: Value::Text(self.name.clone()),
                shown: self.name.clone(),
            }),
            Field::Type => self.type_name.as_ref().map(|name| Reading {
                value: Value::Text(name.clone()),
                shown: name.clone(),
            }),
            Field::Parameter(_) | Field::Quantity(_) | Field::Calculated(_) => {
                self.readings.get(&field.key()).cloned()
            }
        }
    }
}

/// One cell of a row: the value, exact, and the same shown with its unit; both `None` when the
/// element has no value there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The exact value.
    pub value: Option<Value>,
    /// The value shown with its unit.
    pub shown: Option<String>,
}

/// One row: an element, and a cell for each of the schedule's fields, in their order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The element's ID.
    pub element: Id,
    /// The cells, one for each field.
    pub cells: Vec<Cell>,
}

/// A column's heading: the field, what a person calls it and what it holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    /// The field.
    pub field: Field,
    /// Its name as the category's definition has it; the key when the category lacks it.
    pub name: String,
    /// What it holds; `None` when the category lacks the field.
    pub kind: Option<Kind>,
}

/// The columns of a schedule, in the order of its fields.
#[must_use]
pub fn columns(schedule: &Schedule, basis: &Basis<'_>) -> Vec<Column> {
    schedule
        .fields()
        .iter()
        .map(|field| {
            let definition = schedule.definition(basis, field);
            Column {
                field: field.clone(),
                name: definition
                    .as_ref()
                    .map_or_else(|| field.key(), |def| def.name().to_owned()),
                kind: definition.map(|def| def.kind()),
            }
        })
        .collect()
}

/// The order of two rows' values under one sort key: values by exact order, a missing value
/// after every value, whichever way the key runs.
fn by_key(a: Option<&Reading>, b: Option<&Reading>, descending: bool) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => {
            let found = order(&a.value, &b.value).unwrap_or(Ordering::Equal);
            if descending { found.reverse() } else { found }
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// A row with what produced it: the element's facts, calculated fields worked in.
pub(crate) struct Listed {
    /// The row.
    pub(crate) row: Row,
    /// The element's facts with the schedule's calculated fields among its readings.
    pub(crate) subject: Subject,
}

/// The element's facts with each calculated field of the schedule worked out and added.
fn with_calculated(
    schedule: &Schedule,
    basis: &Basis<'_>,
    units: &Units,
    subject: &Subject,
) -> Subject {
    let mut worked = subject.clone();
    for (at, calculated) in schedule.calculated().iter().enumerate() {
        let names = schedule.names_before(basis, at);
        if let Some(reading) = calc::work(calculated, &names, &worked.readings, units) {
            worked.readings.insert(calculated.field().key(), reading);
        }
    }
    worked
}

/// Every row the schedule lists, filtered and sorted, with the facts each came from.
pub(crate) fn listed(
    schedule: &Schedule,
    basis: &Basis<'_>,
    units: &Units,
    subjects: &[Subject],
) -> Vec<Listed> {
    let mut found: Vec<Subject> = subjects
        .iter()
        .filter(|subject| basis.lists(schedule.lists(), subject.category))
        .map(|subject| with_calculated(schedule, basis, units, subject))
        .filter(|subject| {
            schedule.filters().iter().all(|filter| {
                schedule.definition(basis, &filter.field).is_some()
                    && subject
                        .reading(&filter.field)
                        .is_some_and(|reading| filter.holds(&reading.value))
            })
        })
        .collect();
    found.sort_by(|a, b| {
        for sort in schedule.sorting() {
            let found = by_key(
                a.reading(&sort.field).as_ref(),
                b.reading(&sort.field).as_ref(),
                sort.descending,
            );
            if found != Ordering::Equal {
                return found;
            }
        }
        a.id.cmp(&b.id)
    });
    found
        .into_iter()
        .map(|subject| Listed {
            row: Row {
                element: subject.id.clone(),
                cells: schedule
                    .fields()
                    .iter()
                    .map(|field| {
                        let reading = schedule
                            .definition(basis, field)
                            .and_then(|_| subject.reading(field));
                        Cell {
                            value: reading.as_ref().map(|r| r.value.clone()),
                            shown: reading.map(|r| r.shown),
                        }
                    })
                    .collect(),
            },
            subject,
        })
        .collect()
}

/// The rows of a schedule over these elements, one for each element, calculated fields worked out
/// and shown in the metric display settings; [`crate::table`] is the same rows grouped, folded and
/// totalled, shown in the document's own.
#[must_use]
pub fn rows(schedule: &Schedule, basis: &Basis<'_>, subjects: &[Subject]) -> Vec<Row> {
    listed(schedule, basis, &Units::default(), subjects)
        .into_iter()
        .map(|found| found.row)
        .collect()
}
