//! Which category each element kind belongs to.
//!
//! An element kind is the operation name in its recipe (`wall.straight`), as construct-model's
//! FORMAT.md names it; an operation whose owner is not Spare Construct itself is named with its
//! owner in front (`workshop.part`). This crate does not depend on the crates that define the
//! kinds; the table below lists them by name, and the job that adds a kind adds its line here.

use crate::{
    CategoryError, CategoryId, DETAIL_COMPONENTS, DETAIL_LINES, DOORS, FILLED_REGIONS, FLOORS,
    GENERIC_MODELS, GRIDS, LEVELS, MASKING_REGIONS, OPENINGS, REFERENCE_PLANES, ROOFS, ROOMS,
    SHEETS, SPOT_DIMENSIONS, VIEWPORTS, VIEWS, WALLS,
};

/// Every element kind that exists, with its category, sorted by kind name.
///
/// Workshop's parts and assemblies (feature map area CON-A27) are made things that can go into
/// any building, so no category of building parts is theirs: they are generic models.
const KINDS: &[(&str, CategoryId)] = &[
    ("block", GENERIC_MODELS),
    ("detail.component", DETAIL_COMPONENTS),
    ("detail.line", DETAIL_LINES),
    ("detail.linestyle", DETAIL_LINES),
    ("detail.masking", MASKING_REGIONS),
    ("detail.region", FILLED_REGIONS),
    ("detail.repeat", DETAIL_COMPONENTS),
    ("detail.slope", SPOT_DIMENSIONS),
    ("door.single", DOORS),
    // A placed family (job 6.4): its own category is its family's, which its record carries; by
    // its kind alone it is a generic model.
    ("family.instance", GENERIC_MODELS),
    ("floor", FLOORS),
    ("grid.arc", GRIDS),
    ("grid.line", GRIDS),
    ("level", LEVELS),
    ("opening.floor", OPENINGS),
    ("opening.wall", OPENINGS),
    ("refplane.horizontal", REFERENCE_PLANES),
    ("refplane.vertical", REFERENCE_PLANES),
    ("shaft", OPENINGS),
    ("roof", ROOFS),
    ("room", ROOMS),
    ("sheet", SHEETS),
    ("view", VIEWS),
    ("viewport", VIEWPORTS),
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
