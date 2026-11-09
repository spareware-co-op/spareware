//! Why the instance tree could not cache a definition, build an assembly or answer a query.
//!
//! What it does: [`InstanceError`] names every refusal of [`crate::instance`], each with what the
//! caller needs to find its cause: the record, the definition or the side-table entry.
//!
//! Guarantees:
//!
//! - **One kind per cause.** A refusal for a bad input never shares a kind with a defect
//!   ([`InstanceError::Defect`]) or with the meter stopping ([`InstanceError::Stopped`]).
//!
//! Method: none; an error enum and its `Display` and `Error` implementations, as chapter 22 asks
//! of every crate (`projects/engines/geometry/architecture/22-errors-queries-ai.md`).
//!
//! Cost: a few machine words; nothing allocates.

use core::fmt;

use geom_bvh::BvhError;
use geom_math::MathError;
use geom_num::NumError;
use geom_run::{Stop, Stopped};

/// Why the instance tree could not cache a definition, build an assembly or answer a query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstanceError {
    /// The meter stopped: its budget ran out, the caller cancelled, or memory was refused.
    Stopped(Stop),
    /// The box tree refused: a box beyond its limit, too many entries, or the tree's own stop.
    Tree(BvhError),
    /// A record, or a nested assembly, names this handle, which the table of definitions does not
    /// hold.
    NoDefinition(u32),
    /// A general placement names this entry, which the side table of transforms does not hold.
    NoTransform(u32),
    /// A record's placement is not one: a packed permutation past 47.
    Placement,
    /// The record at this index has a box that does not hold its definition as placed.
    Unbounded(u32),
    /// A definition with no geometry: a tree with no parts, or an assembly with no records.
    Empty,
    /// Assemblies nested deeper than [`super::MAX_NESTING`].
    TooDeep,
    /// The frames of a definition and of the assembly placing it have no exact shift between
    /// them: other coordinate reference systems, units, epochs, rotations or scales, or off a
    /// shared lattice (chapter 17.1).
    Frame(MathError),
    /// A general placement's transform has no inverse a query can enter by.
    Transform(MathError),
    /// A coordinate a placement or a frame shift sends out of the grid's range.
    Num(NumError),
    /// More records than an assembly indexes (`u32::MAX`).
    TooManyRecords,
    /// An assembly was given a table of definitions other than the one it was built with: a
    /// handle it holds now names another definition.
    Stale(u32),
    /// An allocation was refused.
    OutOfMemory,
    /// A step the tree's own construction rules out failed: a defect, reported, never answered.
    Defect,
}

impl fmt::Display for InstanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstanceError::Stopped(stop) => write!(f, "the instance tree stopped: {stop}"),
            InstanceError::Tree(e) => write!(f, "the instance tree's box tree: {e}"),
            InstanceError::NoDefinition(h) => {
                write!(f, "the table of definitions holds no definition {h}")
            }
            InstanceError::NoTransform(i) => {
                write!(f, "the side table of transforms holds no entry {i}")
            }
            InstanceError::Placement => f.write_str("a placement that is not one of the 48"),
            InstanceError::Unbounded(r) => write!(
                f,
                "record {r}'s box does not hold its definition as its placement moves it"
            ),
            InstanceError::Empty => f.write_str("a definition with no geometry"),
            InstanceError::TooDeep => write!(
                f,
                "assemblies nested deeper than {} levels",
                super::MAX_NESTING
            ),
            InstanceError::Frame(e) => write!(f, "frames with no exact shift between them: {e}"),
            InstanceError::Transform(e) => write!(f, "a general placement: {e}"),
            InstanceError::Num(e) => write!(f, "{e}"),
            InstanceError::TooManyRecords => f.write_str("more records than an assembly indexes"),
            InstanceError::Stale(h) => write!(
                f,
                "definition {h} is not the one the assembly was built with: another table"
            ),
            InstanceError::OutOfMemory => f.write_str("an allocation was refused"),
            InstanceError::Defect => {
                f.write_str("a step the instance tree's own bounds rule out failed (a defect)")
            }
        }
    }
}

impl core::error::Error for InstanceError {}

impl From<Stopped> for InstanceError {
    fn from(stopped: Stopped) -> Self {
        InstanceError::Stopped(stopped.reason())
    }
}

impl From<BvhError> for InstanceError {
    fn from(e: BvhError) -> Self {
        match e {
            BvhError::Stopped(stop) => InstanceError::Stopped(stop),
            BvhError::OutOfMemory => InstanceError::OutOfMemory,
            other => InstanceError::Tree(other),
        }
    }
}

impl From<NumError> for InstanceError {
    fn from(e: NumError) -> Self {
        InstanceError::Num(e)
    }
}
