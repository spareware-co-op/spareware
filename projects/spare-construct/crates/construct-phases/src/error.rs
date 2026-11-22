//! Why a change or a question about phases and options is refused.

use kit_core::Id;
use std::fmt;

/// A refusal. Every function in this crate returns one as a value; none panics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PhaseError {
    /// A name is empty, or only spaces.
    EmptyName,
    /// An ID is already used by another phase, set or option.
    DuplicateId(Id),
    /// A name is already used by another phase, or by another option in the same set, or by
    /// another set.
    DuplicateName(String),
    /// No phase has this ID.
    UnknownPhase(Id),
    /// No option set has this ID.
    UnknownSet(Id),
    /// The set has no option with this ID.
    UnknownOption {
        /// The set that was asked.
        set: Id,
        /// The option it does not hold.
        option: Id,
    },
    /// A phase was asked to move to a place past the end of the list.
    PositionOutOfRange {
        /// The place asked for, counting from 0.
        position: usize,
        /// How many phases there are.
        len: usize,
    },
    /// The demolished phase comes before the created phase.
    DemolishedBeforeCreated {
        /// The created phase.
        created: Id,
        /// The demolished phase.
        demolished: Id,
    },
    /// A view chose an option twice for the same set.
    DuplicateChoice(Id),
    /// A stored option set holds no options, so it can have no primary.
    EmptySet(Id),
    /// A stored option set's primary is not one of its options.
    PrimaryNotInSet {
        /// The set.
        set: Id,
        /// The primary it names.
        primary: Id,
    },
}

impl fmt::Display for PhaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => f.write_str("a name can't be empty"),
            Self::DuplicateId(id) => write!(f, "the ID {id} is already used"),
            Self::DuplicateName(name) => write!(f, "the name {name:?} is already used"),
            Self::UnknownPhase(id) => write!(f, "no phase has the ID {id}"),
            Self::UnknownSet(id) => write!(f, "no option set has the ID {id}"),
            Self::UnknownOption { set, option } => {
                write!(f, "the option set {set} has no option {option}")
            }
            Self::PositionOutOfRange { position, len } => {
                write!(f, "position {position} is past the end of {len} phases")
            }
            Self::DemolishedBeforeCreated {
                created,
                demolished,
            } => write!(
                f,
                "demolished phase {demolished} comes before created phase {created}"
            ),
            Self::DuplicateChoice(set) => write!(f, "the option set {set} is chosen twice"),
            Self::EmptySet(set) => write!(f, "the option set {set} has no options"),
            Self::PrimaryNotInSet { set, primary } => {
                write!(f, "the primary {primary} is not an option of the set {set}")
            }
        }
    }
}

impl std::error::Error for PhaseError {}
