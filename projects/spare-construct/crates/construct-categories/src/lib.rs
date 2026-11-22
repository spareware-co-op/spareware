//! Spare Construct: categories.
//!
//! The element category tree, as data. Every element has a category (wall, door, floor, room),
//! and a category can have subcategories, so a drawing can style one part of an element on its
//! own (a door's panel, frame and swing). Visibility, line styles, schedules, tags and export
//! mappings all key on the category's permanent id. Map: projects/spare-construct/FEATURES.md,
//! part CON-A2-F2-P6; the stored form is in FORMAT.md.
//!
//! - [`builtin`]: the fixed built-in tree and the constants that name its top-level categories.
//! - [`table`]: a project's table (its own subcategories) and the pure function that adds one.
//! - [`kinds`]: which category an element kind belongs to.
//!
//! Nothing here touches the edit log: a later job wires [`CategoryTable::add_subcategory`] into a
//! command. Nothing here reads a clock or a random number.

pub mod builtin;
pub mod kinds;
pub mod table;

pub use builtin::*;
pub use kinds::category_of_kind;
pub use table::{Category, CategoryError, CategoryTable};

use serde::{Deserialize, Serialize};

/// A category's permanent id. Once a release has named an id it never changes meaning: stored
/// models hold these. Built-in ids are below [`FIRST_USER_ID`]; a project's own start there.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct CategoryId(pub u32);

/// What sort of thing a category holds.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CategoryKind {
    /// A part of the building, or the space in it: walls, doors, rooms.
    Model,
    /// A reference every other element is placed against: levels, grids, reference planes.
    Datum,
    /// A mark drawn over the model in a view: dimensions, tags, text, section markers.
    Annotation,
    /// A page or view of the model: views, sheets, schedules.
    View,
    /// A stand-in made for structural or energy analysis.
    Analytical,
    /// Something brought in from outside: a drawing file, a linked model, a point cloud.
    Import,
}
