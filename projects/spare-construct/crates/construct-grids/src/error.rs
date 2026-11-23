//! Every refusal, as a value the caller can read. A refusal changes nothing.

use construct_params::ParamError;
use kit_core::Id;
use std::fmt;

/// Why a grid, a reference plane or a stored record was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GridError {
    /// A name needs something in it besides spaces.
    EmptyName,
    /// A name is longer than [`crate::MAX_NAME_CHARS`] characters or holds a control character.
    BadName,
    /// Another grid already has this name (case does not matter).
    DuplicateGridName(String),
    /// Another reference plane already has this name (case does not matter).
    DuplicatePlaneName(String),
    /// A grid or a reference plane already has this ID.
    DuplicateId(Id),
    /// No grid has this ID.
    UnknownGrid(Id),
    /// No reference plane has this ID.
    UnknownPlane(Id),
    /// A straight line needs two different points.
    ZeroLength,
    /// An arc needs a radius above zero.
    ZeroRadius,
    /// An arc needs a start that is not its end.
    NoSweep,
    /// A coordinate, a radius or an elevation is further out than the limit
    /// ([`crate::LIMIT`] steps from the origin).
    OutOfRange,
    /// An arc cannot be a full turn or more.
    FullTurn,
    /// The operation is for another shape: a line's ends are points, an arc's are angles.
    WrongShape(&'static str),
    /// A length or an angle was refused by its parameter kind.
    Param(ParamError),
    /// A stored record is not one this crate writes; the text says why.
    Malformed(String),
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "A name needs something in it besides spaces."),
            Self::BadName => write!(
                f,
                "A name is at most {} characters, with no control characters.",
                crate::MAX_NAME_CHARS
            ),
            Self::DuplicateGridName(name) => {
                write!(f, "Another grid is already named {name:?}.")
            }
            Self::DuplicatePlaneName(name) => {
                write!(f, "Another reference plane is already named {name:?}.")
            }
            Self::DuplicateId(id) => write!(f, "{id} is already a grid or a reference plane."),
            Self::UnknownGrid(id) => write!(f, "No grid has the ID {id}."),
            Self::UnknownPlane(id) => write!(f, "No reference plane has the ID {id}."),
            Self::ZeroLength => write!(f, "A line needs two different points."),
            Self::ZeroRadius => write!(f, "An arc needs a radius above zero."),
            Self::NoSweep => write!(f, "An arc needs a start that is not its end."),
            Self::OutOfRange => write!(
                f,
                "That is further than {} mm from the origin.",
                crate::LIMIT
                    .steps()
                    .checked_div(construct_params::Length::STEPS_PER_MM)
                    .unwrap_or(0)
            ),
            Self::FullTurn => write!(f, "An arc cannot be a full turn or more."),
            Self::WrongShape(what) => write!(f, "{what}"),
            Self::Param(e) => write!(f, "{e}"),
            Self::Malformed(why) => {
                write!(f, "This is not a grid or reference plane record: {why}.")
            }
        }
    }
}

impl std::error::Error for GridError {}

impl From<ParamError> for GridError {
    fn from(error: ParamError) -> Self {
        Self::Param(error)
    }
}
