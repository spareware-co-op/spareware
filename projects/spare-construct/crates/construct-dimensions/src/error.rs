//! Every refusal, as a value the caller can read. Its words are what a person sees.

use kit_core::Id;
use std::fmt;

/// Why a dimension, a record of one or a measurement was refused. A refusal changes nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DimensionError {
    /// A dimension measures between at least two references.
    TooFewReferences(usize),
    /// More references than a dimension string may hold.
    TooManyReferences(usize),
    /// Two neighbouring references are the same, so the segment between them has no length.
    RepeatedReference(usize),
    /// A reference position that is not in the string.
    NoSuchReference(usize),
    /// A direction with no length.
    ZeroDirection,
    /// A line, point or direction beyond the kernel's grid.
    OutOfRange(String),
    /// An equality that names a segment that is not there, one twice, or fewer than two.
    BadEquality(String),
    /// Segments are equal by a stored equality: the references cannot change under it.
    HasEqualities,
    /// The layout asked for is not the dimension's.
    WrongLayout(String),
    /// A dimension already has this ID.
    Duplicate(Id),
    /// No dimension has this ID.
    Unknown(Id),
    /// A value that cannot be shown in the person's units.
    Unshowable(String),
    /// A record that is not a dimension as written.
    Malformed(String),
}

impl fmt::Display for DimensionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewReferences(n) => write!(
                f,
                "A dimension measures between at least two references; this has {n}."
            ),
            Self::TooManyReferences(n) => write!(
                f,
                "A dimension string holds at most {} references; this has {n}.",
                crate::MAX_REFERENCES
            ),
            Self::RepeatedReference(at) => write!(
                f,
                "References {} and {} are the same: the segment between them would have no length.",
                at.saturating_add(1),
                at.saturating_add(2)
            ),
            Self::NoSuchReference(at) => {
                write!(
                    f,
                    "There is no reference {} in this dimension.",
                    at.saturating_add(1)
                )
            }
            Self::ZeroDirection => {
                write!(f, "An aligned dimension needs a direction with a length.")
            }
            Self::OutOfRange(why) => write!(f, "Out of range: {why}."),
            Self::BadEquality(why) => write!(f, "Cannot make segments equal: {why}."),
            Self::HasEqualities => write!(
                f,
                "Segments of this dimension are held equal: take the equalities away (dimension.equal with none) before changing its references."
            ),
            Self::WrongLayout(why) | Self::Unshowable(why) | Self::Malformed(why) => {
                f.write_str(why)
            }
            Self::Duplicate(id) => write!(f, "There is already a dimension {id}."),
            Self::Unknown(id) => write!(f, "There is no dimension {id} in this model."),
        }
    }
}

impl std::error::Error for DimensionError {}
