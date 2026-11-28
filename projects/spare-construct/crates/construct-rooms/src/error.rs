//! Every refusal this crate makes, as a value the caller reads. A refusal changes nothing. One
//! about a wall names it by its element's permanent ID. [`RoomError::code`] is the word a build
//! failure is filed under (construct-model's `GeometryError` code), so a page, a script or an AI
//! can tell refusals apart without reading the words.
//!
//! A room that is not enclosed, or that shares its loop with an earlier room, is not refused: it
//! is an outcome ([`crate::Outcome`]), and the record stays.

use kit_core::Id;
use std::fmt;

/// Why a room could not be found, measured or written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoomError {
    /// The room's point is inside a wall's outline, or on it.
    PointInWall {
        /// The wall.
        wall: Id,
    },
    /// The room's point is on a separation line or an area boundary line.
    PointOnLine {
        /// The line.
        line: Id,
    },
    /// A line whose two ends are one point.
    ZeroLine,
    /// A thing that is not what it was taken for in a slab's surface: a plane through points on
    /// one straight line, or no points at all.
    NoPlane,
    /// A curved wall is on the room's level. Until the kernel's exact arcs reach the walls, a
    /// room is never found from a chord standing in for an arc.
    CurvedWall {
        /// The wall.
        wall: Id,
    },
    /// A sloped upper limit (a roof, a sloped ceiling): refused until the kernel can bound a
    /// room's volume by a sloped plane.
    SlopedLimit,
    /// A corner of the room's loop, where two wall faces cross, is not on the length grid. Its
    /// area would not be a whole number of square steps, and it is never rounded.
    OffGrid {
        /// The walls whose faces meet there, ascending.
        walls: Vec<Id>,
    },
    /// The room's area is not a whole number of square grid steps (an odd twice-area), so it
    /// cannot be held exactly.
    AreaNotExact,
    /// A wall's outline has fewer than three corners, or no area.
    NoOutline {
        /// The wall.
        wall: Id,
    },
    /// The room's upper limit is at or below its base.
    NoHeight,
    /// A coordinate, a sum or a product is beyond the kernel's grid or the length type.
    TooBig,
    /// A record or recipe that is not a room's.
    NotARoom(String),
    /// A room's recipe or record that is not as the format says, in words.
    Malformed(String),
    /// The search's own loops did not close. It does not happen; it is a value, not a crash.
    Inconsistent,
}

impl RoomError {
    /// The word a build failure is filed under.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::PointInWall { .. } => "PointInWall",
            Self::PointOnLine { .. } => "PointOnLine",
            Self::ZeroLine => "ZeroLine",
            Self::NoPlane => "NoPlane",
            Self::CurvedWall { .. } => "CurvedWall",
            Self::SlopedLimit => "SlopedLimit",
            Self::OffGrid { .. } => "OffGrid",
            Self::AreaNotExact => "AreaNotExact",
            Self::NoOutline { .. } => "NoOutline",
            Self::NoHeight => "NoHeight",
            Self::TooBig => "TooBig",
            Self::NotARoom(_) => "NotARoom",
            Self::Malformed(_) => "BadRoom",
            Self::Inconsistent => "Inconsistent",
        }
    }
}

fn names(walls: &[Id]) -> String {
    walls
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" and ")
}

impl fmt::Display for RoomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PointInWall { wall } => write!(
                f,
                "The room's point is in wall {wall}: place it in the open space the room is."
            ),
            Self::PointOnLine { line } => write!(
                f,
                "The point is on line {line}: place it on one side of the line, in the space it is."
            ),
            Self::ZeroLine => f.write_str("A line needs two different ends."),
            Self::NoPlane => f.write_str("A floor's top is not a plane the room can be cut by."),
            Self::CurvedWall { wall } => write!(
                f,
                "Wall {wall} is curved, and a room is not found from curved walls yet: the kernel's exact arcs come first."
            ),
            Self::SlopedLimit => f.write_str(
                "A sloped upper limit is not taken yet: give the room a level above or a height.",
            ),
            Self::OffGrid { walls } => write!(
                f,
                "Where the faces of {} meet, the room's corner is off the length grid, so its area would not be exact.",
                names(walls)
            ),
            Self::AreaNotExact => f.write_str(
                "The room's area is not a whole number of square grid steps, so it cannot be held exactly.",
            ),
            Self::NoOutline { wall } => {
                write!(f, "Wall {wall} has no outline in plan with an area.")
            }
            Self::NoHeight => f.write_str("The room's upper limit is at or below its base."),
            Self::TooBig => f.write_str("A length is beyond the kernel's grid."),
            Self::NotARoom(why) => write!(f, "Not a room: {why}."),
            Self::Malformed(why) => write!(f, "The room's record is not as written: {why}."),
            Self::Inconsistent => {
                f.write_str("The room's loops did not close; this is a fault in the search.")
            }
        }
    }
}

impl std::error::Error for RoomError {}
