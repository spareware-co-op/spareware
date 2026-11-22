//! Which category each element kind belongs to.
//!
//! An element kind is the operation name in its recipe (`wall.straight`), as construct-model's
//! FORMAT.md names it. This crate does not depend on the crates that define the kinds; the table
//! below lists them by name, and the job that adds a kind adds its line here.

use crate::{CategoryError, CategoryId, DOORS, GENERIC_MODELS, WALLS};

/// Every element kind that exists, with its category, sorted by kind name.
const KINDS: &[(&str, CategoryId)] = &[
    ("block", GENERIC_MODELS),
    ("door.single", DOORS),
    ("wall.straight", WALLS),
];

/// The category of an element kind, by its exact name.
///
/// # Errors
///
/// [`CategoryError::UnknownKind`] when no element kind has that name.
pub fn category_of_kind(kind: &str) -> Result<CategoryId, CategoryError> {
    KINDS
        .iter()
        .find(|(name, _)| *name == kind)
        .map(|(_, id)| *id)
        .ok_or_else(|| CategoryError::UnknownKind(kind.to_string()))
}
