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
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

/// The current version of another document (20.1): its permanent ID, and the version a reference
/// was made against.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DocRef {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    pub version: u64,
}

/// What a reference wants, when what it named has been split (20.4).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Want {
    One,
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
    #[serde(with = "kit_ops::id")]
    pub element: Id,
    /// The `NameId` of what it points at.
    pub name: String,
    /// What kind of thing that is, such as `face`.
    pub kind: String,
    pub want: Want,
    /// Kept only to suggest a repair. Resolving never guesses from it.
    pub signature: Option<String>,
}

/// One element's data.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Element {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    /// The document it belongs to: a model, in the folders' words.
    #[serde(with = "kit_ops::id")]
    pub document: Id,
    pub kind: String,
    pub name: String,
    pub level: i32,
    pub placement: Placement,
    pub recipe: Recipe,
    pub props: BTreeMap<String, String>,
    pub references: Vec<Reference>,
}
