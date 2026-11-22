//! Spare Construct: materials (CON-A7-F9-P1).
//!
//! A material is a name and a class (concrete, timber, steel, glass, insulation, ...) with three
//! separate groups of data that never touch each other: drawing graphics for projection and for
//! a cut, physical data with a source for every value, and the name of a render look. This crate
//! holds what a material is, a default metric library of common building materials with public
//! sources, the pure changes a project makes, the mass of a volume, and the stored form. It is
//! data and pure functions only; putting a change through the edit log is a later job, and so is
//! the index of what uses a material (a delete is told the users).
//!
//! - [`material`]: [`Material`], [`MaterialId`], [`Class`], [`Drawing`] and [`Graphics`] (a
//!   [`construct_styles::Colour`], a [`construct_styles::PatternId`] and a named area fill, the
//!   place for fill patterns), [`Physical`] and [`Measure`] (density in whole grams per cubic
//!   metre, optional conductivity and specific heat, each with its [`SourceRef`]).
//! - [`library`]: [`Materials`], with `with_material`, `duplicated`, `renamed`, `with_class`,
//!   `with_drawing`, `with_physical`, `with_look`, `without`, the lookups, and `to_bytes` and
//!   `from_bytes`.
//! - [`default`]: the default list and where each value came from.
//! - [`mass`]: [`mass_grams`], volume times density, rounded half to even to a whole gram.
//! - [`error`]: [`MaterialError`], every refusal as a value.
//!
//! Promises: the same library is always the same bytes; ids are permanent; no float is stored;
//! nothing panics on any input. Depends on kit-core (the ID), construct-styles (colour and
//! pattern) and construct-params (the volume). Nothing here is interim.

pub mod default;
pub mod error;
pub mod library;
pub mod mass;
pub mod material;

pub use error::MaterialError;
pub use library::{FORMAT_VERSION, Materials, Source};
pub use mass::mass_grams;
pub use material::{
    Class, Drawing, Graphics, Material, MaterialId, Measure, NewMaterial, Physical, RefName,
    SourceRef,
};
