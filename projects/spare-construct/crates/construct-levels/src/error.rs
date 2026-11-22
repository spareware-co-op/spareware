//! Every refusal, as a value the caller can read. Its words are what a person sees.

use construct_params::Length;
use kit_core::Id;
use std::fmt;

/// Why a level, a tie or a stored record was refused. A refusal changes nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LevelError {
    /// A level needs a name with something in it besides spaces.
    EmptyName,
    /// The document already has a level of this name (case does not matter).
    DuplicateName(String),
    /// A level already has this ID.
    DuplicateLevel(Id),
    /// No level has this ID.
    UnknownLevel(Id),
    /// Another level at this elevation is already a storey.
    StoreyAtElevation {
        /// The level that would be a storey.
        name: String,
        /// The storey already there.
        other: String,
        /// The elevation they share.
        elevation: Length,
    },
    /// Elements are tied to the level, so it stays.
    Tied {
        /// The level's name.
        name: String,
        /// How many elements are tied to it.
        count: usize,
    },
    /// A height worked out from this level does not fit a length.
    OutOfRange(String),
    /// Typed text that is not a length, in construct-params' words.
    Typed(String),
    /// A record that is not a level or a tie as written.
    Malformed(String),
}

impl fmt::Display for LevelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "A level needs a name."),
            Self::DuplicateName(name) => {
                write!(f, "There is already a level named {name:?} in this model.")
            }
            Self::DuplicateLevel(id) => write!(f, "There is already a level {id}."),
            Self::UnknownLevel(id) => write!(f, "There is no level {id} in this model."),
            Self::StoreyAtElevation {
                name,
                other,
                elevation,
            } => write!(
                f,
                "{name} can't be a storey at {elevation}: {other} is already the storey at that elevation."
            ),
            Self::Tied { name, count } => {
                let elements = if *count == 1 {
                    "element is"
                } else {
                    "elements are"
                };
                write!(f, "{name} can't be deleted: {count} {elements} tied to it.")
            }
            Self::OutOfRange(name) => {
                write!(f, "A height from {name} is beyond what a length can hold.")
            }
            Self::Typed(why) | Self::Malformed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for LevelError {}
