//! Spare Construct: schedules, the data and the live query (CON-A10-F1-P1, P2 and P4).
//!
//! A schedule lists elements of one category: a name, the category, its fields in order, its
//! filters, its sorting and grouping, its totals and its calculated fields. It is a query, not a
//! table: [`rows`] and [`table()`] take the document's current elements and give the rows, so after
//! any edit the same call gives the new rows, and nothing about the rows is stored. Map:
//! projects/spare-construct/ARCHITECTURE.md; the stored form is in FORMAT.md beside this crate and
//! the choices are in notes/CON-A10-F1-P1-schedules.md and notes/CON-A10-F1-P2-schedules-b.md.
//!
//! - [`Schedule`]: its ID, name, [`Field`]s, [`Filter`]s, [`Sort`]s (a key with a header or a
//!   footer groups), whether every instance is itemized, its grand total, its [`Totals`] and its
//!   [`Calculated`] fields, written as an element record of kind [`SCHEDULE_KIND`] in the
//!   schedules category and read back strictly; [`Schedule::check`] asks whether it stands in a
//!   document ([`Basis`]).
//! - [`Field`]: `name`, `type`, a parameter by its definition's ID, a read-only quantity by name
//!   ([`QUANTITIES`], which include a sheet's and a view's own facts for the sheet list and the
//!   view list) or a calculated field of the schedule; [`Basis`] says what a category has.
//! - [`Filter`], [`Comparison`]: eight comparisons, each refused where it makes no sense for the
//!   field; every number is compared by its exact value.
//! - [`table()`], [`Table`], [`Item`]: the rows grouped under headers and footers, folded into one
//!   row with a count when instances are not itemized, and totalled ([`TotalKind`]: sum, count,
//!   min, max) exactly, with the grand total.
//! - [`Calculated`]: a formula over other fields, in the project's one formula language
//!   (construct-formulas), exact and checked for units.
//! - [`Schedule::edit_target`], [`EditTarget`]: where a typed value in a cell goes back to, or why
//!   the cell is read-only, in words.
//! - [`SchedulePlacement`], [`Placements`], [`draw()`], [`Drawn`]: a schedule placed on a sheet as
//!   its own element, and its live table laid out as text and lines at exact places.
//! - [`Schedules`]: a document's schedules by ID, names unique without regard to case.
//! - [`rows`], [`columns`], [`Subject`], [`Row`], [`Cell`]: the live query.
//! - [`ScheduleError`]: every refusal, as a value.
//!
//! Promises: records are the truth (main's ruling 7), and this crate is the types and pure
//! functions over them; the commands are construct-api's `cloud::schedules`. The same data gives
//! the same bytes and the same rows on every machine and run (no clock, no random number, no
//! hash-map order; text is ordered by one stated rule, no locale); reading refuses anything
//! writing would not give; no number is held as a float, and totals and formulas are exact;
//! nothing panics. Depends on construct-categories, construct-formulas (the formula language),
//! construct-model (the record), construct-params (values, kinds, definitions), construct-phases
//! (staging) and kit-core (IDs). Nothing here is interim.
//!
//! Not built here, with their places kept:
//! - A revision list: the model has no revisions yet (CON-A11-F1-P4 and CON-A11-F3-P1), so there
//!   is nothing to list; the revision list is a schedule of that category when it exists.
//! - Printing a sheet with its schedule (job 8.7) and a schedule split over several sheets.
//! - A roof's pitch and per-layer quantities as fields (see [`QUANTITIES`]).

mod calc;
mod draw;
mod edit;
mod error;
mod field;
mod filter;
mod placement;
mod rows;
mod schedule;
mod schedules;
mod table;
mod total;

pub use calc::{Calculated, MAX_NAME_LEN, reference_name};
pub use draw::{CHAR_WIDTH, Drawn, DrawnRule, DrawnText, PADDING, ROW_HEIGHT, TEXT_HEIGHT, draw};
pub use edit::EditTarget;
pub use error::ScheduleError;
pub use field::{Basis, Field, QUANTITIES, Quantity};
pub use filter::{Comparison, Filter, order, text_order};
pub use placement::{
    CATEGORY_KEY as PLACEMENT_CATEGORY_KEY, PLACEMENT_KIND, PLACEMENT_NAME, Placements,
    SCHEDULE_KEY, SHEET_KEY, SchedulePlacement, X_KEY, Y_KEY, placement_recipe,
};
pub use rows::{Cell, Column, Reading, Row, Subject, columns, rows};
pub use schedule::{
    CALCULATED_KEY, CATEGORY_KEY, FIELDS_KEY, FILTERS_KEY, GRAND_KEY, ITEMIZE_KEY, LISTS_KEY,
    SCHEDULE_KIND, SORTING_KEY, STAGING_KEY, Schedule, Sort, TOTALS_KEY, schedule_recipe,
};
pub use schedules::Schedules;
pub use table::{Footer, Header, Item, Table, TableRow, Total, table};
pub use total::{TotalKind, Totals, aggregate};
