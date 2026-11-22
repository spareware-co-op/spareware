//! Every refusal this crate makes. A refusal is a value the caller reads; nothing here panics.

use crate::material::MaterialId;
use std::fmt;

/// Why a material, a source, a change or a stored form was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MaterialError {
    /// A material name that is empty after trimming.
    EmptyName,
    /// A name, a source field or a reference name longer than the limit, or holding a control
    /// character.
    BadText {
        /// Which field.
        field: &'static str,
    },
    /// Another material already has this name, compared without regard to case.
    DuplicateName {
        /// The material that has it.
        existing: MaterialId,
    },
    /// Another material already has this ID, or the ID is one of the built-in set's.
    DuplicateId {
        /// The ID.
        id: MaterialId,
    },
    /// No material has this ID.
    NoSuchMaterial {
        /// The ID asked for.
        id: MaterialId,
    },
    /// A physical value of zero or above the largest this crate stores.
    BadValue {
        /// Which value: `density`, `conductivity` or `specific_heat`.
        property: &'static str,
        /// What was given.
        value: u64,
    },
    /// A source reference names a source the library does not hold.
    UnknownSource {
        /// The key.
        key: String,
    },
    /// A source key that is not 1 to 40 of a to z, 0 to 9 and `-`, or a source with an empty field.
    BadSource {
        /// The key as given.
        key: String,
    },
    /// A source with this key is already in the library.
    DuplicateSource {
        /// The key.
        key: String,
    },
    /// The material is in use and so cannot be deleted.
    InUse {
        /// The material.
        id: MaterialId,
        /// What uses it, as the caller named them.
        users: Vec<String>,
    },
    /// A built-in material cannot be deleted (it can be edited; its ID never changes).
    BuiltInNotDeletable {
        /// The material.
        id: MaterialId,
    },
    /// A mass was asked for a volume below zero.
    NegativeVolume,
    /// Stored bytes that are not valid JSON of the shape this crate writes, or hold data that
    /// breaks a rule (the rule's refusal is the message).
    Format(String),
    /// A stored version other than 1.
    Version {
        /// The version found.
        found: u32,
    },
    /// Valid stored data spelt some other way than this crate writes it.
    NotCanonical,
}

impl fmt::Display for MaterialError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(out, "a material needs a name"),
            Self::BadText { field } => {
                write!(out, "{field} is too long or holds a control character")
            }
            Self::DuplicateName { existing } => {
                write!(out, "material {existing} already has that name")
            }
            Self::DuplicateId { id } => write!(out, "material ID {id} is already taken"),
            Self::NoSuchMaterial { id } => write!(out, "no material {id}"),
            Self::BadValue { property, value } => {
                write!(out, "{value} is not a usable {property}")
            }
            Self::UnknownSource { key } => write!(out, "no source {key}"),
            Self::BadSource { key } => write!(out, "source {key} is not valid"),
            Self::DuplicateSource { key } => write!(out, "source {key} already exists"),
            Self::InUse { id, users } => {
                write!(out, "material {id} is used by {} things", users.len())
            }
            Self::BuiltInNotDeletable { id } => {
                write!(out, "built-in material {id} cannot be deleted")
            }
            Self::NegativeVolume => write!(out, "a volume below zero has no mass"),
            Self::Format(why) => write!(out, "not a materials document: {why}"),
            Self::Version { found } => write!(out, "materials format {found} is not known"),
            Self::NotCanonical => write!(out, "materials document is not in its one spelling"),
        }
    }
}

impl std::error::Error for MaterialError {}
