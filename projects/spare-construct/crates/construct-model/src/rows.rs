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

/// What an element's shape reads besides its own record: the derived `reads` row, kept so that a
/// change to what it reads builds it again. Written only when there is something in it.
///
/// Stored in two spellings. An element whose recipe names a parameter, and that neither is hosted
/// nor hosts, keeps the row as it was first written: its type's ID, or `null`. Any other row is an
/// object, `{"parameters":true,"type":...,"host":...,"hosted":[...]}`, each key left out when it
/// is false, `null` or empty. Both spellings read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reads {
    /// Its recipe names a parameter: a change to its type or to the document's parameters builds
    /// it again.
    pub parameters: bool,
    /// The type it reads them through, by ID, when it names one.
    pub type_id: Option<String>,
    /// The element it is hosted on, by ID: a change to that element builds it again, because where
    /// it sits is read from its host.
    pub host: Option<String>,
    /// The elements hosted on it, by ID, in key order: its shape is cut by theirs, so a change to
    /// any of them builds it again.
    pub hosted: Vec<String>,
}

impl Reads {
    /// Whether the row holds nothing, and so is not written.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.parameters && self.type_id.is_none() && self.host.is_none() && self.hosted.is_empty()
    }
}

/// The object spelling of [`Reads`].
#[derive(Serialize, Deserialize)]
struct ReadsObject {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    parameters: bool,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    type_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hosted: Vec<String>,
}

/// Either spelling of [`Reads`], as read.
#[derive(Deserialize)]
#[serde(untagged)]
enum ReadsSpelling {
    Object(ReadsObject),
    Type(Option<String>),
}

impl Serialize for Reads {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.parameters && self.host.is_none() && self.hosted.is_empty() {
            return self.type_id.serialize(serializer);
        }
        ReadsObject {
            parameters: self.parameters,
            type_id: self.type_id.clone(),
            host: self.host.clone(),
            hosted: self.hosted.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Reads {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match ReadsSpelling::deserialize(deserializer)? {
            ReadsSpelling::Type(type_id) => Reads {
                parameters: true,
                type_id,
                ..Reads::default()
            },
            ReadsSpelling::Object(o) => Reads {
                parameters: o.parameters,
                type_id: o.type_id,
                host: o.host,
                hosted: o.hosted,
            },
        })
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn row(reads: &Reads) -> String {
        serde_json::to_string(reads).unwrap_or_default()
    }

    fn read(text: &str) -> Option<Reads> {
        serde_json::from_str(text).ok()
    }

    /// A row written before hosting, a type's ID or `null`, reads as an element that reads
    /// parameters, and such an element that neither hosts nor is hosted writes it the same way.
    #[test]
    fn a_reads_row_from_before_hosting_reads_and_writes_as_it_was() {
        let typed = Reads {
            parameters: true,
            type_id: Some("T-1".to_owned()),
            ..Reads::default()
        };
        assert_eq!(read(r#""T-1""#), Some(typed.clone()));
        assert_eq!(row(&typed), r#""T-1""#);
        let untyped = Reads {
            parameters: true,
            ..Reads::default()
        };
        assert_eq!(read("null"), Some(untyped.clone()));
        assert_eq!(row(&untyped), "null");
    }

    /// A hosted element, and one hosting others, write the object spelling with only what they
    /// hold, and it reads back as written.
    #[test]
    fn a_reads_row_names_a_host_and_what_it_hosts() {
        let door = Reads {
            host: Some("W-1".to_owned()),
            ..Reads::default()
        };
        assert_eq!(row(&door), r#"{"host":"W-1"}"#);
        let wall = Reads {
            parameters: true,
            type_id: Some("T-1".to_owned()),
            hosted: vec!["D-1".to_owned(), "D-2".to_owned()],
            ..Reads::default()
        };
        assert_eq!(
            row(&wall),
            r#"{"parameters":true,"type":"T-1","hosted":["D-1","D-2"]}"#
        );
        for reads in [door, wall] {
            assert_eq!(read(&row(&reads)), Some(reads.clone()));
            assert!(!reads.is_empty());
        }
        assert!(Reads::default().is_empty());
        assert_eq!(read("{}"), Some(Reads::default()));
        assert_eq!(read("7"), None);
    }
}
