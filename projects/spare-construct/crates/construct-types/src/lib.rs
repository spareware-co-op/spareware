//! Spare Construct: element types, stored once (CON-A2-F1-P1).
//!
//! A type is a named variant that its instances share: a door type, a wall type. It holds the
//! values of the parameters its type holds, once; each instance names its type and stores only
//! its own instance values, so changing a type changes what every instance of it reads, and no
//! value is copied into an instance. Map: projects/spare-construct/ARCHITECTURE.md, "Data
//! model"; the stored form is in FORMAT.md beside this crate.
//!
//! - [`ElementType`]: a type's permanent ID, name, category, the parameters its elements have,
//!   and its type values; written as an element record of kind [`TYPE_KIND`] and read back.
//! - [`Types`]: a project's types by ID: add, rename, duplicate and delete, with names unique
//!   within a category without regard to case.
//! - [`Instance`]: the type an element names and its own values, written into and read from its
//!   record's `props`; moving it to another type of its category.
//! - [`resolve`]: the one function that reads an instance's parameter: its own value, then its
//!   type's, then the definition's default.
//! - [`users`]: which of the instances a caller hands over use a type. This crate keeps no index
//!   of instances; the caller (the element store) passes them.
//! - [`Parameters`]: the caller's parameter definitions, each with an optional default.
//! - [`TypeError`]: every refusal, as a value.
//!
//! Promises: records are the truth (main's ruling 7), and this crate is the types and pure
//! functions over them; nothing here touches the edit log (job 2.6 wires these into commands);
//! the same data gives the same bytes; reading refuses anything writing would not give; nothing
//! panics. Depends on construct-model (the record), construct-categories (`CategoryId`),
//! construct-params (definitions and values) and kit-core (permanent IDs). Nothing here is
//! interim.
//!
//! Not built here, with their places kept:
//! - Family types (job 3.6): a type made from a family recipe. Its record is a type record whose
//!   `recipe` names the family recipe and its type row in place of the plain type recipe, read
//!   by a new arm in [`ElementType::from_record`]; the parameter and value rules stay as they are.
//! - System types with layers (job 3.4) and other system data: a type may carry one
//!   [`SystemData`] block, an owner tag (such as `wall`) and that owner's canonical text, which
//!   this crate stores, returns and compares byte for byte and never reads inside. The layers
//!   themselves are the owning crate's (construct-walls); nothing of them is here.
//! - Type catalogues: rows of a supplier's sizes, each read into [`ElementType::new`] and
//!   [`ElementType::set`]; no new stored form.
//! - Default values on definitions: [`Parameters::add`] takes a definition and an optional
//!   default, the shape a project parameter binding in construct-params carries, so a binding
//!   feeds it directly and [`resolve`] does not change.

mod element_type;
mod error;
mod instance;
mod parameters;
mod record;
mod resolve;
mod system;
mod types;

pub use element_type::ElementType;
pub use error::TypeError;
pub use instance::{Instance, users};
pub use parameters::Parameters;
pub use record::{
    CATEGORY_KEY, PARAMS_KEY, SYSTEM_KEY_PREFIX, TYPE_KEY, TYPE_KIND, system_type_recipe,
    type_recipe,
};
pub use resolve::{Resolved, Source, resolve};
pub use system::SystemData;
pub use types::Types;
