//! What Spare Construct stores besides elements: its project spaces, and the shapes it derives.
//! Project spaces and teams are CON-A25-F1, packages CON-A25-F2, taking in CON-A25-F4, folders
//! CON-A26-F1. Part of the published format (FORMAT.md).

use crate::geometry::{Bounds, Edition, FaceName};
use kit_core::Id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Hub {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Project {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    pub name: String,
    #[serde(with = "kit_ops::id")]
    pub hub: Id,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Team {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    pub name: String,
}

/// A document in a team's WIP folder: the header every document has.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Model {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    pub name: String,
    #[serde(with = "kit_ops::id")]
    pub team: Id,
    /// What publishing it makes, as in "Arch package".
    pub package: String,
    /// What builds its shapes. Changes only through an ordered upgrade.
    pub edition: Edition,
}

/// How someone belongs to a hub.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HubRole {
    User,
    External,
}

/// Where a consumed package came from.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Origin {
    #[serde(with = "kit_ops::id")]
    pub hub: Id,
    pub hub_name: String,
    #[serde(with = "kit_ops::id")]
    pub project: Id,
    #[serde(with = "kit_ops::id")]
    pub team: Id,
    #[serde(with = "kit_ops::id")]
    pub package: Id,
}

/// A numbered package in a Shared or Consumed folder. The bytes are a file; this says where it is
/// and what its hash must be.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Item {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    pub name: String,
    pub version: u32,
    #[serde(with = "kit_ops::id")]
    pub model: Id,
    /// The log number the package froze the model at.
    pub seq: u64,
    pub file: String,
    pub sha256: String,
    pub elements: usize,
    #[serde(with = "kit_ops::id")]
    pub by: Id,
    pub from: Option<Origin>,
    pub bridged_to: Vec<String>,
    /// The log entry that made this package.
    pub entry: u64,
}

/// A package bridged to a hub, waiting for the team to take it into Consumed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Incoming {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    #[serde(with = "kit_ops::id")]
    pub to_team: Id,
    pub item: Item,
    pub origin: Origin,
}

/// The three ways a model can link another.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    /// Follows a model in the same team's WIP.
    Live,
    /// A fixed package from a Shared folder.
    Shared,
    /// A fixed package from the team's Consumed folder.
    Consumed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Link {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    pub kind: LinkKind,
    pub name: String,
    /// The model a live link follows.
    #[serde(with = "kit_ops::id::option")]
    pub model: Option<Id>,
    /// The team and package a fixed link points at.
    #[serde(with = "kit_ops::id::option")]
    pub team: Option<Id>,
    #[serde(with = "kit_ops::id::option")]
    pub package: Option<Id>,
}

/// What a checkpoint entry says: the bodies regenerated since the last one, as the server built them
/// (note A05). A browser compares the ones it built itself, for any element it holds no newer edit of.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    /// The log number these were regenerated through.
    pub through: u64,
    /// Body content IDs by element.
    pub bodies: BTreeMap<String, String>,
}

/// Where a derived record's bytes are: in the store while they're small, in file storage, linked
/// by hash, when they're large.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Stored {
    Here { hex: String },
    File { path: String, size: u64 },
}

/// One level of detail of a built element, by content ID.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DisplayRef {
    pub lod: u32,
    pub deviation: i64,
    #[serde(with = "kit_ops::id")]
    pub id: Id,
}

/// What regeneration made of an element. Derived: delete every one of these and regeneration
/// brings back the same bytes. Never the source of truth.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BuiltElement {
    /// The input record this was built from.
    #[serde(with = "kit_ops::id")]
    pub input: Id,
    pub edition: Edition,
    pub level: i32,
    /// The 5 m cells it was indexed under.
    pub cells: Vec<(i64, i64)>,
    pub result: BuiltResult,
}

/// A built shape, or why there isn't one. A failure is model state, the same on every machine.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BuiltResult {
    Shape {
        #[serde(with = "kit_ops::id")]
        body: Id,
        bounds: Bounds,
        names: Vec<FaceName>,
        displays: Vec<DisplayRef>,
    },
    Failed {
        code: String,
        message: String,
    },
}
