//! Spare Kit: core types every app uses.
//!
//! Permanent 128-bit IDs, units, errors and common types.
//! Map: root ARCHITECTURE.md, "Spare Kit".
//!
//! This crate has no dependencies and must keep it that way, optional ones included: the geometry
//! kernel's facade depends on it, and the kernel's merge gate reads everything it reaches. So `Id`
//! carries no `serde` here; `kit-ops` holds the serde helpers.

mod id;

pub use id::{Id, IdError};
