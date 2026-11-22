//! Spare Construct: typed parameters with units, shared and project parameters (CON-A2-F2-P1, P3, P5, CON-A2-F3-P1).
//!
//! A parameter is a named value on an element or on its type, with a kind. This crate holds what
//! one is and how its values are kept, shown and read; it is data and pure functions only, and
//! setting a value on an element goes through the edit log elsewhere. Map:
//! projects/spare-construct/ARCHITECTURE.md, "Data model"; the stored form is in FORMAT.md beside
//! this crate, and why values are whole numbers is note CON-A2-F3-P1.
//!
//! - [`Definition`]: a parameter's permanent ID, name, [`Kind`], and whether it belongs to the
//!   instance or the type ([`Holder`]).
//! - [`Value`]: one value. A [`Length`] is whole grid steps of the geometry kernel (`Q0`,
//!   1/6,400 mm, so it hands to the kernel unrounded and 12.5 mm is exact), an [`Area`] whole
//!   square steps and a [`Volume`] whole cubic steps (128 bits), angles whole angle units of the
//!   geometry kernel, slopes a ratio of whole numbers, numbers exact decimals. No float is
//!   stored. Typed text off the grid (0.001 mm) is refused naming the nearest values, never
//!   rounded.
//! - [`Units`]: how values are shown and read: metric, a [`Format`] per measured kind.
//! - [`Values`]: an element's or a type's values by parameter ID, into and out of the element
//!   record's `props`.
//! - [`ParamError`]: every refusal, as a value naming the parameter and the kinds that met.
//!
//! - [`SharedParameter`] and [`SharedFile`]: a parameter with a UUID the caller gives, and the shared
//!   parameter file a project exports and another imports (CON-A2-F2-P3).
//! - [`ProjectParams`]: a project's parameters bound to categories of `construct-categories` as
//!   instance or type, with an optional default value, and which parameters an element of a
//!   category has (CON-A2-F2-P3).
//! - [`ProjectInfo`]: project name, number, client, address, stage and issue date, as parameters
//!   of the project itself, with extras (CON-A2-F2-P5).
//!
//! Promises: the same values give the same bytes on every machine; every stored value has one
//! spelling; nothing panics on any input. Depends on kit-core and kit-ops for permanent IDs, and on
//! `rust_decimal`, `num-rational` and `url`. Nothing here is interim.
//!
//! One units system: this crate owns the program's quantities, kinds, units ([`Unit`] is the one
//! table), typed text and display. The formula language is `construct-formulas`, which works in
//! these kinds and values and depends on this crate, never the reverse: a [`Definition`] may carry
//! a formula's text, and `construct-formulas` reads, checks and works it out. Centimetres, inches
//! and feet are in the table for formulas only; typing and showing them is a later job.

mod definition;
mod error;
mod info;
mod project;
mod quantity;
mod shared;
mod text;
mod units;
mod value;
mod values;

pub use definition::{Choice, Definition, Holder, Kind};
pub use error::{Op, ParamError};
pub use info::{InfoError, InfoField, InfoRow, IssueDate, ProjectInfo, is_builtin_id};
pub use num_rational::Ratio;
pub use project::{Applicable, HolderValues, ProjectError, ProjectParams, Unbound, can_hold};
pub use quantity::{Angle, Area, Length, Slope, Volume};
pub use rust_decimal::Decimal;
pub use shared::{SharedError, SharedFile, SharedParameter, is_uuid};
pub use units::{Format, Unit, Units};
pub use value::Value;
pub use values::Values;
