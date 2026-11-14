//! What Spare Construct stores besides elements: its project spaces, and the shapes it derives.
//! Project spaces and teams are CON-A25-F1, packages CON-A25-F2, taking in CON-A25-F4, folders
//! CON-A26-F1. Part of the published format (FORMAT.md).

use crate::geometry::{Bounds, Edition, FaceName};
use kit_core::Id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A hub: one firm's space, holding its projects (CON-A25-F1).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Hub {
    /// Its permanent ID.
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    /// The name people see.
    pub name: String,
}

/// A project in a hub.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Project {
    /// Its permanent ID.
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    /// The name people see.
    pub name: String,
    /// The hub it belongs to.
    #[serde(with = "kit_ops::id")]
    pub hub: Id,
}

/// A team in a project, with its own WIP, Shared and Consumed folders.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Team {
    /// Its permanent ID.
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    /// The name people see.
    pub name: String,
}

/// A document in a team's WIP folder: the header every document has.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Model {
    /// Its permanent ID.
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    /// The name people see.
    pub name: String,
    /// The team whose WIP folder holds it.
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
    /// A member of the hub's own firm.
    User,
    /// Someone from another firm, let in to particular projects.
    External,
}

/// Where a consumed package came from.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Origin {
    /// The hub it was shared in.
    #[serde(with = "kit_ops::id")]
    pub hub: Id,
    /// That hub's name, as it was then.
    pub hub_name: String,
    /// The project it was shared in.
    #[serde(with = "kit_ops::id")]
    pub project: Id,
    /// The team that shared it.
    #[serde(with = "kit_ops::id")]
    pub team: Id,
    /// The package in that team's Shared folder.
    #[serde(with = "kit_ops::id")]
    pub package: Id,
}

/// A numbered package in a Shared or Consumed folder. The bytes are a file; this says where it is
/// and what its hash must be.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Item {
    /// Its permanent ID.
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    /// The name people see.
    pub name: String,
    /// Its number among the packages made from the same model.
    pub version: u32,
    /// The model it was made from.
    #[serde(with = "kit_ops::id")]
    pub model: Id,
    /// The log number the package froze the model at.
    pub seq: u64,
    /// Where its bytes are in file storage.
    pub file: String,
    /// The SHA-256 its bytes must have, in hex.
    pub sha256: String,
    /// How many elements it holds.
    pub elements: usize,
    /// The person who made it.
    #[serde(with = "kit_ops::id")]
    pub by: Id,
    /// Where it came from, for a package taken in from another hub.
    pub from: Option<Origin>,
    /// The hubs it has been bridged to, by name.
    pub bridged_to: Vec<String>,
    /// The log entry that made this package.
    pub entry: u64,
}

/// A package bridged to a hub, waiting for the team to take it into Consumed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Incoming {
    /// Its permanent ID.
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    /// The team it waits for.
    #[serde(with = "kit_ops::id")]
    pub to_team: Id,
    /// The package.
    pub item: Item,
    /// Where it came from.
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

/// A link from a model to another model, or to a package.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Link {
    /// Its permanent ID.
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    /// Which of the three ways it links.
    pub kind: LinkKind,
    /// The name people see.
    pub name: String,
    /// The model a live link follows.
    #[serde(with = "kit_ops::id::option")]
    pub model: Option<Id>,
    /// The team and package a fixed link points at.
    #[serde(with = "kit_ops::id::option")]
    pub team: Option<Id>,
    /// The package a fixed link points at, in that team's folder.
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
    /// In the store.
    Here {
        /// The bytes, in hex.
        hex: String,
    },
    /// In file storage.
    File {
        /// Where, named by the bytes' hash.
        path: String,
        /// How many bytes.
        size: u64,
    },
}

/// One level of detail of a built element, by content ID.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DisplayRef {
    /// 0 is the finest.
    pub lod: u32,
    /// How far this level strays from the body, in millimetres.
    pub deviation: i64,
    /// The display record, by the content ID of its bytes.
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
    /// What built it.
    pub edition: Edition,
    /// The element's level.
    pub level: i32,
    /// The 5 m cells it was indexed under.
    pub cells: Vec<(i64, i64)>,
    /// The shape, or why there isn't one.
    pub result: BuiltResult,
}

/// A built shape, or why there isn't one. A failure is model state, the same on every machine.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BuiltResult {
    /// It built.
    Shape {
        /// The body record, by the content ID of its bytes.
        #[serde(with = "kit_ops::id")]
        body: Id,
        /// A box around the body.
        bounds: Bounds,
        /// The persistent names of its faces.
        names: Vec<FaceName>,
        /// Its levels of detail, finest first.
        displays: Vec<DisplayRef>,
    },
    /// It didn't build.
    Failed {
        /// A short code that names what went wrong.
        code: String,
        /// What went wrong, in words for people.
        message: String,
    },
}
