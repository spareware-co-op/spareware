//! Shared parameters: a parameter whose permanent ID is a UUID the caller gives, so the same
//! parameter means the same thing in two projects and in a family, and the shared parameter file
//! a project exports and another imports (CON-A2-F2-P3).
//!
//! A [`SharedParameter`] is a [`Definition`] whose ID is a UUID in its one spelling (lower case,
//! hyphenated, not the nil UUID), with the name of the group it is filed under. A [`SharedFile`]
//! is a set of them. [`SharedFile::add`] and [`SharedFile::import`] are pure: they return a new
//! file, importing a file already imported changes nothing, and the same ID met again with a
//! different kind (or any other different field) is refused. Nothing here makes a UUID or reads
//! a clock: the caller gives the ID. The file's stored form is in FORMAT.md.

use crate::definition::{Definition, Kind};
use kit_core::Id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Why a shared parameter or a shared parameter file was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SharedError {
    /// The ID is not a UUID in its one spelling (lower case, hyphenated, not all zero).
    BadUuid(String),
    /// A group name must have something in it and no space at either end.
    BadGroup(String),
    /// The ID is already in the file as a parameter of another kind.
    KindConflict {
        /// The parameter's ID.
        id: Id,
        /// The name of the parameter already in the file.
        name: String,
        /// The kind it already has.
        existing: Kind,
        /// The kind that was offered.
        incoming: Kind,
    },
    /// The ID is already in the file with a different name, group, holder or list of choices.
    Conflict {
        /// The parameter's ID.
        id: Id,
        /// The name of the parameter already in the file.
        name: String,
        /// Which field differs.
        field: &'static str,
    },
    /// Another parameter in the same group has this name (case does not matter).
    DuplicateName {
        /// The group.
        group: String,
        /// The name.
        name: String,
    },
    /// The stored file is not a version this build reads.
    UnsupportedFormat(u32),
    /// The stored file is not valid; the text says why.
    Malformed(String),
}

impl fmt::Display for SharedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadUuid(text) => write!(
                f,
                "{text:?} is not a UUID (lower case, hyphenated, not all zero)"
            ),
            Self::BadGroup(text) => {
                write!(
                    f,
                    "{text:?} is not a group name: it needs text and no outer spaces"
                )
            }
            Self::KindConflict {
                id,
                name,
                existing,
                incoming,
            } => write!(
                f,
                "{} is already {name:?}, {existing}, and cannot also be {incoming}",
                id.as_str()
            ),
            Self::Conflict { id, name, field } => write!(
                f,
                "{} is already {name:?} and its {field} differs",
                id.as_str()
            ),
            Self::DuplicateName { group, name } => {
                write!(f, "group {group:?} already has a parameter named {name:?}")
            }
            Self::UnsupportedFormat(v) => {
                write!(f, "shared parameter file format {v} is not supported")
            }
            Self::Malformed(why) => write!(f, "shared parameter file is not valid: {why}"),
        }
    }
}

impl std::error::Error for SharedError {}

/// Whether `text` is a UUID in its one spelling: lower case, hyphenated, not all zero.
#[must_use]
pub fn is_uuid(text: &str) -> bool {
    match uuid::Uuid::parse_str(text) {
        Ok(parsed) => !parsed.is_nil() && parsed.hyphenated().to_string() == text,
        Err(_) => false,
    }
}

/// The first field in which two definitions, with the groups they are filed under, differ; `None`
/// when they are the same. `"kind"` is checked first.
pub(crate) fn difference(
    a: &Definition,
    a_group: Option<&str>,
    b: &Definition,
    b_group: Option<&str>,
) -> Option<&'static str> {
    if a.kind() != b.kind() {
        Some("kind")
    } else if a.name() != b.name() {
        Some("name")
    } else if a_group != b_group {
        Some("group")
    } else if a.holder() != b.holder() {
        Some("holder")
    } else if a.choices() != b.choices() {
        Some("choices")
    } else {
        None
    }
}

/// A parameter shared across projects and families: a definition whose ID is a UUID, and the
/// group it is filed under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedParameter {
    definition: Definition,
    group: String,
}

impl SharedParameter {
    /// Makes a shared parameter of `definition`, filed under `group`.
    ///
    /// # Errors
    /// [`SharedError::BadUuid`] if the definition's ID is not a UUID in its one spelling, or
    /// [`SharedError::BadGroup`] if the group is empty or has a space at either end.
    pub fn new(definition: Definition, group: &str) -> Result<Self, SharedError> {
        if !is_uuid(definition.id().as_str()) {
            return Err(SharedError::BadUuid(definition.id().as_str().to_owned()));
        }
        if group.is_empty() || group.trim() != group {
            return Err(SharedError::BadGroup(group.to_owned()));
        }
        Ok(Self {
            definition,
            group: group.to_owned(),
        })
    }

    /// The parameter's definition; its ID is the UUID.
    #[must_use]
    pub fn definition(&self) -> &Definition {
        &self.definition
    }

    /// The group it is filed under.
    #[must_use]
    pub fn group(&self) -> &str {
        &self.group
    }

    /// Its permanent ID, the UUID.
    #[must_use]
    pub fn id(&self) -> &Id {
        self.definition.id()
    }
}

/// A set of shared parameters: what a project exports and another imports.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SharedFile {
    parameters: BTreeMap<Id, SharedParameter>,
}

/// The file's format this build writes (FORMAT.md). Earlier formats are read for ever.
const FORMAT: u32 = 1;

/// One parameter as stored: the group, then the definition's own row.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireParameter {
    group: String,
    definition: Definition,
}

/// The stored form: field order here is the byte order.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    format: u32,
    parameters: Vec<WireParameter>,
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl SharedFile {
    /// A file with no parameters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The parameter with this ID.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&SharedParameter> {
        self.parameters.get(id)
    }

    /// How many parameters.
    #[must_use]
    pub fn len(&self) -> usize {
        self.parameters.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }

    /// Every parameter in the file's one order: by group, then name, then ID.
    #[must_use]
    pub fn parameters(&self) -> Vec<&SharedParameter> {
        let mut all: Vec<&SharedParameter> = self.parameters.values().collect();
        all.sort_by(|a, b| {
            (a.group(), a.definition.name(), a.id()).cmp(&(b.group(), b.definition.name(), b.id()))
        });
        all
    }

    /// The file with one more parameter. A parameter already there, the same in every field, leaves
    /// the file as it was. This file is not changed.
    ///
    /// # Errors
    /// [`SharedError::KindConflict`] if the ID is already there with another kind,
    /// [`SharedError::Conflict`] if it differs in any other field, or
    /// [`SharedError::DuplicateName`] if another parameter in the group has the name.
    pub fn add(&self, parameter: SharedParameter) -> Result<Self, SharedError> {
        if let Some(existing) = self.parameters.get(parameter.id()) {
            return match difference(
                &existing.definition,
                Some(&existing.group),
                &parameter.definition,
                Some(&parameter.group),
            ) {
                None => Ok(self.clone()),
                Some("kind") => Err(SharedError::KindConflict {
                    id: parameter.id().clone(),
                    name: existing.definition.name().to_owned(),
                    existing: existing.definition.kind(),
                    incoming: parameter.definition.kind(),
                }),
                Some(field) => Err(SharedError::Conflict {
                    id: parameter.id().clone(),
                    name: existing.definition.name().to_owned(),
                    field,
                }),
            };
        }
        if self.parameters.values().any(|other| {
            other.group == parameter.group
                && same_name(other.definition.name(), parameter.definition.name())
        }) {
            return Err(SharedError::DuplicateName {
                group: parameter.group,
                name: parameter.definition.name().to_owned(),
            });
        }
        let mut next = self.clone();
        next.parameters.insert(parameter.id().clone(), parameter);
        Ok(next)
    }

    /// The file with every parameter of `other` added, as [`SharedFile::add`] adds one. Importing
    /// a file that was imported before changes nothing. All or nothing: on a refusal the caller
    /// keeps this file as it was.
    ///
    /// # Errors
    /// The first refusal of [`SharedFile::add`], in the order of [`SharedFile::parameters`].
    pub fn import(&self, other: &Self) -> Result<Self, SharedError> {
        other
            .parameters()
            .into_iter()
            .try_fold(self.clone(), |file, parameter| file.add(parameter.clone()))
    }

    /// The file's stored bytes: the same parameters always give the same bytes (FORMAT.md).
    ///
    /// # Errors
    /// [`SharedError::Malformed`] if the text cannot be written, which a valid file never hits.
    pub fn to_bytes(&self) -> Result<Vec<u8>, SharedError> {
        let wire = Wire {
            format: FORMAT,
            parameters: self
                .parameters()
                .into_iter()
                .map(|p| WireParameter {
                    group: p.group.clone(),
                    definition: p.definition.clone(),
                })
                .collect(),
        };
        serde_json::to_vec(&wire).map_err(|e| SharedError::Malformed(e.to_string()))
    }

    /// Reads a stored file. Every rule of [`SharedFile::add`] is checked again, and the bytes must
    /// be the one spelling [`SharedFile::to_bytes`] writes, so stored bytes cannot break a rule.
    ///
    /// # Errors
    /// [`SharedError::UnsupportedFormat`] for a format this build does not know, otherwise
    /// [`SharedError::Malformed`] or the rule that an entry broke.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SharedError> {
        let wire: Wire =
            serde_json::from_slice(bytes).map_err(|e| SharedError::Malformed(e.to_string()))?;
        if wire.format != FORMAT {
            return Err(SharedError::UnsupportedFormat(wire.format));
        }
        let mut file = Self::new();
        for entry in wire.parameters {
            let parameter = SharedParameter::new(entry.definition, &entry.group)?;
            if file.parameters.contains_key(parameter.id()) {
                return Err(SharedError::Malformed(format!(
                    "{} appears twice",
                    parameter.id().as_str()
                )));
            }
            file = file.add(parameter)?;
        }
        if file.to_bytes()? != bytes {
            return Err(SharedError::Malformed(
                "not in its one spelling (order, spacing or escapes)".to_owned(),
            ));
        }
        Ok(file)
    }
}
