//! Which category each element kind belongs to.
//!
//! An element kind is the operation name in its recipe (`wall.straight`), as construct-model's
//! FORMAT.md names it; an operation whose owner is not Spare Construct itself is named with its
//! owner in front (`workshop.part`). This crate does not depend on the crates that define the
//! kinds; the table below lists them by name, and the job that adds a kind adds its line here.

use crate::{
    CategoryError, CategoryId, DOORS, GENERIC_MODELS, GRIDS, LEVELS, REFERENCE_PLANES, WALLS,
};

/// Every element kind that exists, with its category, sorted by kind name.
///
/// Workshop's parts and assemblies (feature map area CON-A27) are made things that can go into
/// any building, so no category of building parts is theirs: they are generic models.
const KINDS: &[(&str, CategoryId)] = &[
    ("block", GENERIC_MODELS),
    ("door.single", DOORS),
    ("grid.arc", GRIDS),
    ("grid.line", GRIDS),
    ("level", LEVELS),
    ("refplane.horizontal", REFERENCE_PLANES),
    ("refplane.vertical", REFERENCE_PLANES),
    ("wall.layered", WALLS),
    ("wall.straight", WALLS),
    ("workshop.assembly", GENERIC_MODELS),
    ("workshop.part", GENERIC_MODELS),
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
