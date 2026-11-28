//! Every refusal this crate makes, as a value the caller reads. A refusal changes nothing. A
//! refusal about a grid line names its direction and where it stands (a vertical line by its
//! distance along the wall from its start, a horizontal one by its height above the wall's base);
//! one about a panel names the corner its cell starts at.

use crate::grid::Direction;
use construct_categories::CategoryId;
use construct_params::Length;
use construct_types::TypeError;
use kit_core::Id;
use std::fmt;

/// Why a curtain wall type, a curtain wall, an edit or a stored text was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CurtainError {
    /// A stored text or a recipe argument is not in its one spelling, or an ID does not read.
    Malformed(String),
    /// construct-types refused the type.
    Type(TypeError),
    /// The type is not of the curtain walls category.
    NotACurtainType {
        /// Its category.
        category: CategoryId,
    },
    /// The type has no curtain wall block.
    NoCurtainBlock {
        /// The type.
        ty: Id,
    },
    /// The element is not a curtain wall.
    NotACurtainWall(String),
    /// A size that must be above zero is not.
    NotPositive {
        /// What it is, in words.
        what: String,
        /// Its value.
        value: Length,
    },
    /// A size is larger than this crate takes.
    TooLarge {
        /// What it is, in words.
        what: String,
        /// Its value.
        value: Length,
        /// The most it may be.
        most: Length,
    },
    /// A size whose half the grid cannot hold, so it cannot stand centred on its line.
    OddSteps {
        /// What it is, in words.
        what: String,
        /// Its value.
        value: Length,
    },
    /// A fixed count of panels of none, or of more than [`crate::MOST_COUNT`].
    BadCount {
        /// The direction its lines run.
        direction: Direction,
        /// The count.
        count: u32,
    },
    /// A layout gives more lines than [`crate::MOST_LINES`] in one direction, or a curtain wall
    /// more cells than [`crate::MOST_CELLS`].
    TooMany {
        /// What there would be too many of, in words.
        what: String,
        /// How many there would be.
        count: u64,
        /// The most there may be.
        most: u64,
    },
    /// Equal divisions narrower than the model's tolerance.
    TooFine {
        /// The direction its lines run.
        direction: Direction,
        /// How many divisions.
        count: i64,
    },
    /// A grid line at or past a border, or nearer one than the model's tolerance.
    LineOutside {
        /// The direction it runs.
        direction: Direction,
        /// Where it was asked for.
        at: Length,
    },
    /// A grid line where one already is, or nearer one than the model's tolerance.
    LineTaken {
        /// The direction it runs.
        direction: Direction,
        /// Where it was asked for.
        at: Length,
        /// The line already there.
        near: Length,
    },
    /// No grid line of that direction stands there.
    NoSuchLine {
        /// The direction.
        direction: Direction,
        /// Where it was looked for.
        at: Length,
    },
    /// The line has no segment that starts at that crossing.
    NoSuchSegment {
        /// The line's direction.
        direction: Direction,
        /// The line.
        at: Length,
        /// The crossing the segment was to start at.
        from: Length,
    },
    /// The segment is already left out.
    AlreadyOmitted {
        /// The line's direction.
        direction: Direction,
        /// The line.
        at: Length,
        /// The crossing it starts at.
        from: Length,
    },
    /// The segment is not left out, so it cannot be put back.
    NotOmitted {
        /// The line's direction.
        direction: Direction,
        /// The line.
        at: Length,
        /// The crossing it starts at.
        from: Length,
    },
    /// No panel's cell starts at that corner.
    NoSuchPanel {
        /// The corner: along, then up.
        at: [Length; 2],
    },
    /// Leaving segments out would make a panel that is not a rectangle of cells.
    PanelNotRectangle {
        /// The corner its first cell starts at.
        at: [Length; 2],
    },
    /// A panel between its mullions has no width or no height.
    NoRoomForPanel {
        /// The corner its cell starts at.
        at: [Length; 2],
    },
    /// A mullion between the mullions that cross it has no length.
    NoRoomForMullion {
        /// Its line's direction.
        direction: Direction,
        /// Its line.
        at: Length,
        /// The crossing its segment starts at.
        from: Length,
    },
    /// A door panel's middle falls between two grid steps.
    DoorOffGrid {
        /// The corner its cell starts at.
        at: [Length; 2],
    },
    /// A door panel's family or type text does not read.
    BadDoor(String),
    /// A sum or a product beyond what a length holds.
    OutOfRange,
}

impl From<TypeError> for CurtainError {
    fn from(error: TypeError) -> Self {
        Self::Type(error)
    }
}

/// The words for a corner.
fn corner([along, up]: &[Length; 2]) -> String {
    format!("{along} along and {up} up")
}

impl fmt::Display for CurtainError {
    #[allow(
        clippy::too_many_lines,
        reason = "one sentence for each refusal, in the order they are declared"
    )]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(why) => write!(f, "not a curtain wall in its one spelling: {why}"),
            Self::Type(error) => write!(f, "{error}"),
            Self::NotACurtainType { category } => write!(
                f,
                "a curtain wall type is of the curtain walls category, not category {}",
                category.0
            ),
            Self::NoCurtainBlock { ty } => {
                write!(f, "type {ty} holds no curtain wall grid and parts")
            }
            Self::NotACurtainWall(what) => write!(f, "{what} is not a curtain wall"),
            Self::NotPositive { what, value } => {
                write!(f, "the {what} is {value}: it must be above 0")
            }
            Self::TooLarge { what, value, most } => {
                write!(f, "the {what} is {value}: it may be at most {most}")
            }
            Self::OddSteps { what, value } => write!(
                f,
                "the {what} is {value}, whose half falls between two steps of the length grid, \
                 so it cannot stand centred"
            ),
            Self::BadCount { direction, count } => write!(
                f,
                "a {direction} grid of {count} panels: a fixed count is from 1 to {}",
                crate::MOST_COUNT
            ),
            Self::TooMany { what, count, most } => {
                write!(f, "{count} {what}: a curtain wall holds at most {most}")
            }
            Self::TooFine { direction, count } => write!(
                f,
                "{count} equal {direction} divisions would be narrower than the model's tolerance"
            ),
            Self::LineOutside { direction, at } => write!(
                f,
                "a {direction} grid line at {at} is not inside the curtain wall: it stands more \
                 than 0.01 mm inside its borders"
            ),
            Self::LineTaken {
                direction,
                at,
                near,
            } => write!(
                f,
                "a {direction} grid line at {at} would be within 0.01 mm of the one at {near}"
            ),
            Self::NoSuchLine { direction, at } => {
                write!(f, "there is no {direction} grid line at {at}")
            }
            Self::NoSuchSegment {
                direction,
                at,
                from,
            } => write!(
                f,
                "the {direction} grid line at {at} has no segment starting at {from}"
            ),
            Self::AlreadyOmitted {
                direction,
                at,
                from,
            } => write!(
                f,
                "the segment of the {direction} grid line at {at} from {from} is already left out"
            ),
            Self::NotOmitted {
                direction,
                at,
                from,
            } => write!(
                f,
                "the segment of the {direction} grid line at {at} from {from} is not left out"
            ),
            Self::NoSuchPanel { at } => {
                write!(f, "no panel's cell starts at {}", corner(at))
            }
            Self::PanelNotRectangle { at } => write!(
                f,
                "the panel at {} would not be a rectangle of cells: every panel is one; leave \
                 out the segments that make it one, or keep this one",
                corner(at)
            ),
            Self::NoRoomForPanel { at } => write!(
                f,
                "the panel at {} has no room between its mullions",
                corner(at)
            ),
            Self::NoRoomForMullion {
                direction,
                at,
                from,
            } => write!(
                f,
                "the mullion on the {direction} grid line at {at} from {from} has no room \
                 between the mullions that cross it"
            ),
            Self::DoorOffGrid { at } => write!(
                f,
                "the door panel at {} has its middle between two steps of the length grid",
                corner(at)
            ),
            Self::BadDoor(why) => write!(f, "a door panel: {why}"),
            Self::OutOfRange => write!(f, "a length is beyond what a curtain wall holds"),
        }
    }
}

impl std::error::Error for CurtainError {}
