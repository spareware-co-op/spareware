//! What a dimension reports when a reference cannot be used: a warning as data, in the shape
//! construct-warnings gives every warning (CON-A2-F7-P1): a stable kind name, how serious it is,
//! the elements involved by permanent ID in a stated order, and a sentence for people.
//!
//! A dimension's warning is construct-warnings' own [`Warning`], and [`Severity`] is its too:
//! this crate adds the three kinds' names and nothing else. The same three names are entries of
//! construct-warnings' `RULES` table, which lists what [`crate::measure`] reported for each
//! dimension of a model (construct-api's `cloud::warnings` hands it over).

pub use construct_warnings::{Severity, Warning};

/// The kind of a warning about a reference that is not there or has no place in the view.
pub const REFERENCE_MISSING: &str = "dimension.reference_missing";
/// The kind of a warning about a line reference that runs along the dimension's offset line, so
/// that it has no place along it.
pub const REFERENCE_PARALLEL: &str = "dimension.reference_parallel";
/// The kind of a warning about a value or a point that does not fit the kernel's grid.
pub const OUT_OF_RANGE: &str = "dimension.out_of_range";
