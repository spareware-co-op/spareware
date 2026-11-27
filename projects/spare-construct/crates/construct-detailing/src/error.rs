//! Every refusal, as a value the caller can read. Its words are what a person sees.

use kit_core::Id;
use std::fmt;

/// Why a detail item, a record of one, or a reading of one was refused. A refusal changes nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DetailError {
    /// A point beyond the kernel's grid.
    OutOfRange(String),
    /// A straight line whose two ends are one point.
    ZeroLength,
    /// An arc whose centre, start and end do not make one: the words are the solver's.
    BadArc(String),
    /// An outline with fewer than three corners.
    FewCorners(usize),
    /// An outline with more corners than a region holds.
    TooManyCorners(usize),
    /// A corner given twice in a row (the corner's number from 1).
    RepeatedCorner(usize),
    /// Two edges of an outline that cross, touch or overlap (their numbers from 1).
    SelfCrossing {
        /// The first edge.
        first: usize,
        /// The second edge.
        second: usize,
    },
    /// An outline whose corners are all on one line.
    NoArea,
    /// A line style that is not there.
    UnknownStyle(String),
    /// A line style that cannot be made.
    BadStyle(String),
    /// A fill pattern that is not in the table.
    UnknownPattern(String),
    /// A detail component that is not in the library.
    UnknownComponent(String),
    /// A spacing that is not above zero, or a line that has none.
    BadSpacing(String),
    /// A repeating component that would be placed more times than one may be.
    TooMany(u64),
    /// A face to read a slope from that has none to read.
    Slope(SlopeRefusal),
    /// An item with this ID already exists.
    Duplicate(Id),
    /// No item has this ID.
    Unknown(Id),
    /// A component recipe that is not as written, or breaks a rule of its checker.
    BadRecipe(String),
    /// A record that is not a detail item as written.
    Malformed(String),
}

/// Why a face gives no slope, by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlopeRefusal {
    /// There is no such floor or roof now (deleted, or not a floor or a roof).
    Missing(String),
    /// The point is not over any face of it.
    OffFace(String),
    /// The face is level: it has no slope to read.
    Flat,
    /// The face stands upright: it has no run.
    Vertical,
}

impl fmt::Display for SlopeRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(why) | Self::OffFace(why) => f.write_str(why),
            Self::Flat => f.write_str("The face is flat: it has no slope to read."),
            Self::Vertical => f.write_str("The face is upright: it has no run, so no slope."),
        }
    }
}

impl SlopeRefusal {
    /// The stable name of this kind of refusal: `missing`, `off-face`, `flat` or `vertical`.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Missing(_) => "missing",
            Self::OffFace(_) => "off-face",
            Self::Flat => "flat",
            Self::Vertical => "vertical",
        }
    }
}

impl fmt::Display for DetailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange(why)
            | Self::BadArc(why)
            | Self::BadStyle(why)
            | Self::BadRecipe(why)
            | Self::BadSpacing(why)
            | Self::Malformed(why) => f.write_str(why),
            Self::ZeroLength => f.write_str("A line needs two different ends."),
            Self::FewCorners(n) => {
                write!(f, "An outline needs at least three corners; this has {n}.")
            }
            Self::TooManyCorners(n) => write!(
                f,
                "An outline holds at most {} corners; this has {n}.",
                crate::MAX_CORNERS
            ),
            Self::RepeatedCorner(at) => {
                write!(f, "Corner {at} is the same point as the corner before it.")
            }
            Self::SelfCrossing { first, second } => write!(
                f,
                "The outline crosses itself: edges {first} and {second} cross, touch or overlap."
            ),
            Self::NoArea => f.write_str("The outline has no area: its corners are on one line."),
            Self::UnknownStyle(name) => write!(f, "There is no line style {name:?}."),
            Self::UnknownPattern(name) => write!(f, "There is no fill pattern {name:?}."),
            Self::UnknownComponent(key) => {
                write!(f, "There is no detail component {key:?} in the library.")
            }
            Self::TooMany(n) => write!(
                f,
                "That would place the component {n} times; at most {} at once.",
                crate::MOST_REPEATS
            ),
            Self::Slope(why) => why.fmt(f),
            Self::Duplicate(id) => write!(f, "There is already a detail item {id}."),
            Self::Unknown(id) => write!(f, "There is no detail item {id} in this model."),
        }
    }
}

impl std::error::Error for DetailError {}
