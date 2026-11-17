//! An element, as stored: inputs only. Its shape isn't here: `recipe` says how to build it, and
//! the built shape is a derived record, rebuilt whenever it's wanted. So this is what the edit log
//! changes and what is hashed, and a package or a replay needs nothing else to reach the same model.
//!
//! Placement sits outside the recipe, as the kernel's instance records do (19.12), so every door of
//! one type shares one body, and moving an element rebuilds nothing.

use crate::geometry::Recipe;
use kit_core::Id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Where an element's shape sits, in whole millimetres, within its level.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    /// Along the model's x axis.
    pub x: i64,
    /// Along the model's y axis.
    pub y: i64,
    /// Up, from the level.
    pub z: i64,
}

/// The current version of another document (20.1): its permanent ID, and the version a reference
/// was made against.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DocRef {
    /// The document's permanent ID.
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    /// The version of it the reference was made against.
    pub version: u64,
}

/// What a reference wants, when what it named has been split (20.4).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Want {
    /// One piece: resolving asks which, rather than guessing.
    One,
    /// Every piece it was split into.
    AllPieces,
    /// The underlying plane or surface: the default for hosting, so an opening cut through a face
    /// doesn't break what's hosted on it.
    Carrier,
}

/// A reference to part of another element's shape, by persistent name (20.4). A door is hosted on
/// a face of a wall, not on the wall.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    /// Empty for this document; otherwise the linked document.
    pub doc: Option<DocRef>,
    /// The element whose shape it points into.
    #[serde(with = "kit_ops::id")]
    pub element: Id,
    /// The `NameId` of what it points at.
    pub name: String,
    /// What kind of thing that is, such as `face`.
    pub kind: String,
    /// What it wants if that has been split.
    pub want: Want,
    /// Kept only to suggest a repair. Resolving never guesses from it.
    pub signature: Option<String>,
}

/// One element's data.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Element {
    /// Its permanent ID.
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    /// The document it belongs to: a model, in the folders' words.
    #[serde(with = "kit_ops::id")]
    pub document: Id,
    /// What it is, such as `wall` or `door`.
    pub kind: String,
    /// The name people see.
    pub name: String,
    /// The level it sits on.
    pub level: i32,
    /// Where its shape sits on that level.
    pub placement: Placement,
    /// How to build its shape.
    pub recipe: Recipe,
    /// Its other properties, by name.
    pub props: BTreeMap<String, String>,
    /// What it refers to in other elements' shapes, such as the face a door is hosted on.
    pub references: Vec<Reference>,
}
