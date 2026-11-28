//! A schedule as a table: its rows grouped, folded and totalled, a live query like [`crate::rows`]
//! and, like it, never stored.
//!
//! [`table`] takes a schedule and what is known of each element and gives the items of the
//! table in order:
//!
//! - **Grouping.** A sort key with a header or a footer groups ([`crate::Sort::groups`]): the rows
//!   that agree on that key and on every key before it are one group. Groups nest in the order of
//!   the sort keys. A group's header is the key's field and its value (empty for rows that have
//!   none); its footer is how many elements it holds and the totals of the columns that have
//!   them. A schedule with no grouping key has neither.
//! - **Itemize every instance.** On (the default) every element is a row. Off, the rows of one
//!   group that agree in every column are folded into one, in the order their first row comes,
//!   with a count of the elements it stands for; totals still count every element, so a folded
//!   row of three doors adds three doors' lengths.
//! - **Totals.** For each column that has them, a sum, a count, a minimum and a maximum
//!   ([`crate::TotalKind`]), worked out exactly from the values of the elements in the group (or
//!   in the whole table, for the grand total), never from the shown text.
//!
//! The order is the schedule's sorting, ties in element ID order, so the same elements always give
//! the same table.

use crate::field::{Basis, Field};
use crate::rows::{Cell, Column, Listed, columns, listed};
use crate::schedule::{Schedule, Sort};
use crate::total::{TotalKind, aggregate};
use construct_params::{Definition, Units, Value};
use kit_core::Id;
use std::collections::BTreeMap;

/// A group's header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// How deep the group is among the grouping keys: 0 for the first.
    pub level: usize,
    /// The sort key's field.
    pub field: Field,
    /// The field's name.
    pub name: String,
    /// The group's value shown; `None` for the rows that have no value for the key.
    pub shown: Option<String>,
}

/// One row of the table: an element, or the elements folded into it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableRow {
    /// The first element the row stands for.
    pub element: Id,
    /// Every element the row stands for, in table order; one when every instance is itemized.
    pub elements: Vec<Id>,
    /// How many elements: `elements.len()`.
    pub count: usize,
    /// One cell for each column.
    pub cells: Vec<Cell>,
}

/// One total of one column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Total {
    /// The column, by its place among the schedule's fields.
    pub column: usize,
    /// Which total.
    pub kind: TotalKind,
    /// Its exact value; `None` when no element has a value there (a count is `0` then).
    pub value: Option<Value>,
    /// The value shown in the document's units.
    pub shown: Option<String>,
    /// Why it could not be worked out (a sum too large to hold), in words.
    pub problem: Option<String>,
}

/// A group's footer, or the grand total.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Footer {
    /// The group's depth among the grouping keys; `None` for the grand total.
    pub level: Option<usize>,
    /// The key's field name; empty for the grand total.
    pub name: String,
    /// The group's value shown; `None` for none, and for the grand total.
    pub shown: Option<String>,
    /// How many elements the group holds.
    pub elements: usize,
    /// The totals of the columns that have them.
    pub totals: Vec<Total>,
}

/// One item of the table, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    /// A group begins.
    Header(Header),
    /// A row.
    Row(TableRow),
    /// A group ends.
    Footer(Footer),
}

/// The table a schedule makes of the elements.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    /// The columns, in the order of the schedule's fields.
    pub columns: Vec<Column>,
    /// Headers, rows and footers, in order.
    pub items: Vec<Item>,
    /// The grand total, when the schedule has one.
    pub grand: Option<Footer>,
    /// How many elements the schedule lists.
    pub elements: usize,
    /// Whether every instance has a row; when not, rows carry a count of the elements they stand for.
    pub itemize: bool,
}

/// The value of a sort key for a row, if it has one.
fn key_value(found: &Listed, sort: &Sort) -> Option<Value> {
    found.subject.reading(&sort.field).map(|r| r.value)
}

/// Whether two rows are in one group at a grouping key: they agree on it and on every key before.
fn together(sorting: &[Sort], upto: usize, a: &Listed, b: &Listed) -> bool {
    sorting
        .iter()
        .take(upto.saturating_add(1))
        .all(|sort| key_value(a, sort) == key_value(b, sort))
}

struct Context<'a> {
    schedule: &'a Schedule,
    basis: &'a Basis<'a>,
    units: &'a Units,
    /// The places among the sorting of the keys that group.
    grouping: Vec<usize>,
}

/// The shown form of a value of a field, in the document's units.
fn show(definition: &Definition, value: &Value, units: &Units) -> Option<String> {
    definition.format(value, units).ok()
}

impl Context<'_> {
    fn totals(&self, rows: &[Listed]) -> Vec<Total> {
        let mut out = Vec::new();
        for totals in self.schedule.totals() {
            let Some(column) = self
                .schedule
                .fields()
                .iter()
                .position(|field| *field == totals.field)
            else {
                continue;
            };
            let definition = self.schedule.definition(self.basis, &totals.field);
            for kind in &totals.kinds {
                let values = rows
                    .iter()
                    .filter_map(|found| found.row.cells.get(column)?.value.as_ref());
                let (value, problem) = match aggregate(*kind, values) {
                    Ok(value) => (value, None),
                    Err(e) => (None, Some(e.to_string())),
                };
                let shown = value.as_ref().and_then(|value| match (kind, &definition) {
                    (TotalKind::Count, _) => Some(match value {
                        Value::Whole(n) => n.to_string(),
                        other => other.stored(),
                    }),
                    (_, Some(definition)) => show(definition, value, self.units),
                    _ => None,
                });
                out.push(Total {
                    column,
                    kind: *kind,
                    value,
                    shown,
                    problem,
                });
            }
        }
        out
    }

    fn footer(&self, level: Option<usize>, rows: &[Listed]) -> Footer {
        let (name, shown) = level
            .and_then(|level| self.grouping.get(level))
            .and_then(|at| self.schedule.sorting().get(*at))
            .map_or_else(
                || (String::new(), None),
                |sort| self.key_of(sort, rows.first()),
            );
        Footer {
            level,
            name,
            shown,
            elements: rows.len(),
            totals: self.totals(rows),
        }
    }

    /// The name of a sort key's field and the shown value of the first row of a group.
    fn key_of(&self, sort: &Sort, first: Option<&Listed>) -> (String, Option<String>) {
        let name = self
            .schedule
            .definition(self.basis, &sort.field)
            .map_or_else(
                || sort.field.key(),
                |definition| definition.name().to_owned(),
            );
        (
            name,
            first
                .and_then(|found| found.subject.reading(&sort.field))
                .map(|r| r.shown),
        )
    }

    fn leaves(&self, rows: &[Listed], out: &mut Vec<Item>) {
        if self.schedule.itemize() {
            for found in rows {
                out.push(Item::Row(TableRow {
                    element: found.row.element.clone(),
                    elements: vec![found.row.element.clone()],
                    count: 1,
                    cells: found.row.cells.clone(),
                }));
            }
            return;
        }
        // Folded: rows that agree in every column are one row, where the first of them came.
        let mut at: BTreeMap<Vec<Option<String>>, usize> = BTreeMap::new();
        let mut folded: Vec<TableRow> = Vec::new();
        for found in rows {
            let key: Vec<Option<String>> = found
                .row
                .cells
                .iter()
                .map(|cell| cell.value.as_ref().map(Value::stored))
                .collect();
            if let Some(place) = at.get(&key).and_then(|place| folded.get_mut(*place)) {
                place.elements.push(found.row.element.clone());
                place.count = place.elements.len();
            } else {
                at.insert(key, folded.len());
                folded.push(TableRow {
                    element: found.row.element.clone(),
                    elements: vec![found.row.element.clone()],
                    count: 1,
                    cells: found.row.cells.clone(),
                });
            }
        }
        out.extend(folded.into_iter().map(Item::Row));
    }

    fn emit(&self, rows: &[Listed], depth: usize, out: &mut Vec<Item>) {
        let Some(place) = self.grouping.get(depth) else {
            self.leaves(rows, out);
            return;
        };
        let Some(sort) = self.schedule.sorting().get(*place) else {
            self.leaves(rows, out);
            return;
        };
        let sorting = self.schedule.sorting();
        for run in rows.chunk_by(|a, b| together(sorting, *place, a, b)) {
            if sort.header {
                let (name, shown) = self.key_of(sort, run.first());
                out.push(Item::Header(Header {
                    level: depth,
                    field: sort.field.clone(),
                    name,
                    shown,
                }));
            }
            self.emit(run, depth.saturating_add(1), out);
            if sort.footer {
                out.push(Item::Footer(self.footer(Some(depth), run)));
            }
        }
    }
}

/// The table of a schedule over these elements, its numbers shown in `units`.
#[must_use]
pub fn table(
    schedule: &Schedule,
    basis: &Basis<'_>,
    units: &Units,
    subjects: &[crate::rows::Subject],
) -> Table {
    let found = listed(schedule, basis, units, subjects);
    let context = Context {
        schedule,
        basis,
        units,
        grouping: schedule
            .sorting()
            .iter()
            .enumerate()
            .filter(|(_, sort)| sort.groups())
            .map(|(at, _)| at)
            .collect(),
    };
    let mut items = Vec::new();
    context.emit(&found, 0, &mut items);
    Table {
        columns: columns(schedule, basis),
        items,
        grand: schedule
            .grand_totals()
            .then(|| context.footer(None, &found)),
        elements: found.len(),
        itemize: schedule.itemize(),
    }
}
