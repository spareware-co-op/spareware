//! Every way an export refuses, as a value the caller can read.

use std::fmt;

/// Why a model could not be written to a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IfcError {
    /// A length, a count or an identifier number did not fit its integer.
    TooBig,
    /// An element names a level the export was not given.
    UnknownStorey(String),
    /// Two levels or two elements were given with one ID.
    Duplicate(String),
    /// An element's shape cannot be written, in words.
    Shape(String),
}

impl fmt::Display for IfcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IfcError::TooBig => write!(f, "a length or a count is too big to write"),
            IfcError::UnknownStorey(id) => {
                write!(f, "it stands on level {id}, which was not given")
            }
            IfcError::Duplicate(id) => write!(f, "{id} was given twice"),
            IfcError::Shape(why) => write!(f, "its shape cannot be written: {why}"),
        }
    }
}

impl std::error::Error for IfcError {}
