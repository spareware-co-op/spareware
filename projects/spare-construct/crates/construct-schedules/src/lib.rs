//! Spare Construct: schedules, the data and the live query (CON-A10-F1-P1, first half).
//!
//! A schedule lists elements of one category: a name, the category, its fields in order, its
//! filters and its sorting. It is a query, not a table: [`rows`] takes the document's current
//! elements and gives the rows, so after any edit the same call gives the new rows, and nothing
//! about the rows is stored. Map: projects/spare-construct/ARCHITECTURE.md; the stored form is in
//! FORMAT.md beside this crate and the choices are in notes/CON-A10-F1-P1-schedules.md.
//!
//! - [`Schedule`]: its ID, name, [`Field`]s, [`Filter`]s and [`Sort`]s, written as an element
//!   record of kind [`SCHEDULE_KIND`] in the schedules category and read back strictly;
//!   [`Schedule::check`] asks whether it stands in a document ([`Basis`]).
//! - [`Field`]: `name`, `type`, a parameter by its definition's ID, or a read-only quantity by
//!   name ([`QUANTITIES`]); [`Basis`] says what a category has.
//! - [`Filter`], [`Comparison`]: eight comparisons, each refused where it makes no sense for the
//!   field; every number is compared by its exact value.
//! - [`Schedules`]: a document's schedules by ID, names unique without regard to case.
//! - [`rows`], [`columns`], [`Subject`], [`Row`], [`Cell`]: the live query.
//! - [`ScheduleError`]: every refusal, as a value.
//!
//! Promises: records are the truth (main's ruling 7), and this crate is the types and pure
//! functions over them; the commands are construct-api's `cloud::schedules`. The same data gives
//! the same bytes and the same rows on every machine and run (no clock, no random number, no
//! hash-map order; text is ordered by one stated rule, no locale); reading refuses anything
//! writing would not give; no number is held as a float; nothing panics. Depends on
//! construct-categories, construct-model (the record), construct-params (values, kinds,
//! definitions), construct-phases (staging) and kit-core (IDs). Nothing here is interim.
//!
//! Not built here, with their places kept:
//! - Grouping, totals, formula columns, editing a value in a schedule and the sheet, view and
//!   revision lists (job 8.8, part CON-A10-F1-P1 rest, P2, P4).
//! - Drawing a schedule on a sheet (a later sheets job) and the page.
//! - A roof's pitch and per-layer quantities as fields (see [`QUANTITIES`]).

mod error;
mod field;
mod filter;
mod rows;
mod schedule;
mod schedules;

pub use error::ScheduleError;
pub use field::{
    Basis, FLOOR_AREA, FLOOR_MASS, FLOOR_PERIMETER, FLOOR_THICKNESS, FLOOR_VOLUME, Field,
    QUANTITIES, Quantity, ROOF_AREA, ROOF_MASS, ROOF_RIDGE_HEIGHT, ROOF_SLOPE_AREA, ROOF_THICKNESS,
    ROOF_VOLUME, ROOM_AREA, ROOM_PERIMETER, ROOM_STATE, ROOM_VOLUME, WALL_AREA, WALL_LENGTH,
    WALL_VOLUME,
};
pub use filter::{Comparison, Filter, order, text_order};
pub use rows::{Cell, Column, Reading, Row, Subject, columns, columns_among, rows, rows_among};
pub use schedule::{
    CATEGORY_KEY, FIELDS_KEY, FILTERS_KEY, LISTS_KEY, SCHEDULE_KIND, SORTING_KEY, STAGING_KEY,
    Schedule, Sort, schedule_recipe,
};
pub use schedules::Schedules;
