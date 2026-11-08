//! The instance record as the instance tree reads it: the definition it places, its placement and
//! its integer box, through a trait the caller's record implements (19.12; piece G295).
//!
//! What it does: [`Record`] is what the instance tree needs of chapter 19.12's one instance record,
//! `geom_topo::placed::Placed` (piece G145), which this crate's row of chapter 27's crate table does
//! not let it name: the handle of its definition, its [`Place`] and its box. The caller implements
//! the trait on its record (a wrapper of `Placed` in a crate that sees both), so this crate holds
//! no second record: the tree's leaves are record indices, and every box, placement and definition
//! is read from the caller's records when the tree is built and when it answers.
//!
//! Guarantees:
//!
//! - **No second record.** Nothing in [`crate::instance`] stores an instance's definition,
//!   placement or box beside the caller's record.
//!
//! Method: chapter 19.12's definitions and instances, as IFC's mapped items place a shared
//! representation (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`;
//! evidence report 08, section C:
//! `projects/kit/notes/G01-evidence/08-standards-coverage-testing.md`).
//!
//! Cost: none here; each read is the caller's record's.

use geom_math::{Box3, GridTransform};

use super::error::InstanceError;

/// Where an instance goes, as the instance tree reads it (19.12).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    /// A whole-step shift with one of the 48 signed axis permutations: exact.
    Exact(GridTransform),
    /// An index into the document's side table of general transforms.
    General(u32),
}

/// What the instance tree reads of one instance record (19.12): implemented on the caller's
/// record, chapter 19.12's `Placed`.
pub trait Record {
    /// The handle of the definition it places, in the caller's
    /// [`Definitions`](super::Definitions).
    fn definition(&self) -> u32;

    /// Its placement.
    ///
    /// # Errors
    ///
    /// [`InstanceError::Placement`] for a placement that is not one, such as a packed permutation
    /// past 47.
    fn place(&self) -> Result<Place, InstanceError>;

    /// Its integer box, in the steps of the frame of the assembly that holds it: held once, in the
    /// record.
    fn bound(&self) -> &Box3;
}
