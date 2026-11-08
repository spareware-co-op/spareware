//! Why an arena could not change, or arenas could not be published as a body.
//!
//! What it does: [`TopoError`] names each failure of the arenas ([`crate::column`],
//! [`crate::entity`], [`crate::arenas`]) and of publishing them as a body
//! ([`crate::builder`]), and maps `geom-run`'s stops onto them, so a caller sees one error type.
//!
//! Guarantees:
//!
//! - **Typed and placed.** Every failure is one of these kinds, and each kind that comes from one
//!   row names the arena or link column and the row, so a caller can find it without searching.
//! - **Stops keep their reason.** A meter's stop (budget, cancellation, memory) keeps `geom-run`'s
//!   own record of it.
//!
//! Method: an error enum and its conversions, with `geom-run`'s stops
//! (`projects/engines/geometry/architecture/23-performance.md`, 23.10).
//!
//! Cost: constant.

use core::fmt;

use alloc::collections::TryReserveError;
use geom_run::{Stop, Stopped, WorkMeter};

use crate::entity::Link;
use crate::ix::Arena;

/// Why an Euler operator refused its arguments ([`crate::builder::Euler`]). A refusal changes
/// nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// A handle names no live row of its arena.
    Dead {
        /// The arena.
        arena: Arena,
        /// The row.
        row: u32,
    },
    /// The fins are not in the loops the operator needs: one loop (`mef`, `kemr`), or two loops of
    /// one face (`mekr`).
    Loops,
    /// The edge does not have exactly two fins, each the other's radial neighbour.
    Ring,
    /// The topology is not the shape the operator works on: the edge is not a spur (`kev`) or a
    /// lone wire (`kill_wire`), its fins are next to each other (`kemr`), the operator would leave
    /// a loop with no fin, or a fin it starts from has no vertex.
    Shape,
    /// The faces are not what the operator needs: fins of two faces (`kef`), two faces of one
    /// shell (`kfmrh`), a face to remove with only the loop given (`kef`, `kfmrh`), or a face
    /// that keeps a loop (`mfkrh`).
    Faces,
    /// The region still has shells, or the shell still has other faces.
    InUse,
    /// The body was published without names, so it cannot be edited by name.
    Unnamed,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Dead { arena, row } => write!(f, "{arena} {row} is not a live row"),
            Refusal::Loops => f.write_str("the fins are not in the loops the operator needs"),
            Refusal::Ring => f.write_str("the edge does not have exactly two fins"),
            Refusal::Shape => f.write_str("the topology is not the shape the operator works on"),
            Refusal::Faces => f.write_str("the faces are not what the operator needs"),
            Refusal::InUse => f.write_str("the region or shell still holds other entities"),
            Refusal::Unnamed => f.write_str("the body has no names to edit it by"),
        }
    }
}

/// Why an arena could not change, or arenas could not be published as a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TopoError {
    /// The arena already holds `u32::MAX` rows, the most a 32-bit index addresses.
    Full(Arena),
    /// The arena has no row at this index.
    NoSuchRow {
        /// The arena.
        arena: Arena,
        /// The row asked for.
        row: u32,
    },
    /// The columns of one arena hold different numbers of rows.
    Ragged(Arena),
    /// A link column holds an index past the end of the arena it points into.
    Dangling {
        /// The link column.
        link: Link,
        /// The row of the link column that holds it.
        row: u32,
    },
    /// The body's outside region is not a void region of its region arena.
    Outside,
    /// The meter stopped: its budget ran out, the caller cancelled, or memory was refused.
    Stopped(Stop),
    /// A large allocation (a chunk of rows, or a column's list of chunks) was refused.
    OutOfMemory,
    /// The names given for a named arena are not one per row.
    Names(Arena),
    /// Two rows of a named arena have the same name: the first such pair in name order.
    DuplicateName {
        /// The arena.
        arena: Arena,
        /// The lower of the two rows.
        first: u32,
        /// The higher of the two rows.
        second: u32,
    },
    /// Two rows of an arena with no names (shells, loops, fins) or of a geometry pool get the same
    /// key from what they join, so name order cannot place them: the first such pair.
    Unordered {
        /// The arena.
        arena: Arena,
        /// The lower of the two rows.
        first: u32,
        /// The higher of the two rows.
        second: u32,
    },
    /// An Euler operator refused its arguments.
    Euler(Refusal),
    /// A row of a geometry pool that no entity uses, so name order cannot place it.
    Unused {
        /// The pool.
        arena: Arena,
        /// The row.
        row: u32,
    },
}

impl fmt::Display for TopoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TopoError::Full(arena) => {
                write!(
                    f,
                    "the {arena} arena already holds the most rows an index addresses"
                )
            }
            TopoError::NoSuchRow { arena, row } => {
                write!(f, "the {arena} arena has no row {row}")
            }
            TopoError::Ragged(arena) => {
                write!(
                    f,
                    "the columns of the {arena} arena hold different numbers of rows"
                )
            }
            TopoError::Dangling { link, row } => write!(
                f,
                "{link} of {} {row} points past the end of the {} arena",
                link.from(),
                link.to()
            ),
            TopoError::Outside => {
                f.write_str("the outside region is not a void region of the body")
            }
            TopoError::Stopped(stop) => write!(f, "publishing the body stopped: {stop}"),
            TopoError::OutOfMemory => f.write_str("an allocation was refused"),
            TopoError::Names(arena) => {
                write!(f, "the names of the {arena} arena are not one per row")
            }
            TopoError::DuplicateName {
                arena,
                first,
                second,
            } => write!(f, "{arena} rows {first} and {second} have the same name"),
            TopoError::Unordered {
                arena,
                first,
                second,
            } => write!(
                f,
                "{arena} rows {first} and {second} join the same names, so name order cannot place them"
            ),
            TopoError::Euler(refusal) => write!(f, "the Euler operator refused: {refusal}"),
            TopoError::Unused { arena, row } => {
                write!(f, "no entity uses row {row} of the {arena} pool")
            }
        }
    }
}

impl TopoError {
    /// The stop of `meter` this error stands for: the meter's own for [`TopoError::Stopped`], and
    /// out of memory, recorded on the meter, for [`TopoError::OutOfMemory`]; `None` for any other
    /// error, which no budget or allocator caused.
    pub fn stop(&self, meter: &mut WorkMeter<'_>) -> Option<Stopped> {
        match self {
            TopoError::OutOfMemory => Some(meter.out_of_memory()),
            TopoError::Stopped(_) => meter.check().err(),
            _ => None,
        }
    }
}

impl From<Stopped> for TopoError {
    fn from(stopped: Stopped) -> Self {
        TopoError::Stopped(stopped.reason())
    }
}

impl From<TryReserveError> for TopoError {
    fn from(_: TryReserveError) -> Self {
        TopoError::OutOfMemory
    }
}
