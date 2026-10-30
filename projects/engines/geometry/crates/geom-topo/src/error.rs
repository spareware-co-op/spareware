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
//! Method: none; an error enum and its conversions, with `geom-run`'s stops (chapter 23.10).
//!
//! Cost: constant.

use core::fmt;

use alloc::collections::TryReserveError;
use geom_run::{Stop, Stopped};

use crate::entity::Link;
use crate::ix::Arena;

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
