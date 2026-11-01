//! The canonical rebuild: merged pieces published as a body (11.3, step 5).
//!
//! What it does: [`rebuild`] takes the pieces a selection keeps, has [`merge`] make them faces,
//! edges and shells, and has [`publish`] write those into a body.
//!
//! Guarantees:
//!
//! - **The same bytes however the boxes fell.** Every row comes in the order [`merge`] numbers the
//!   shape, which depends on the shape alone (11.3; tested).
//! - **Refused, never guessed.** What [`merge`] and [`publish`] refuse is a [`RebuildError`].
//!
//! Method: chapter 11.3's rebuild (`projects/engines/geometry/architecture/11-booleans.md`), after
//! EMBER (Trettner, Nehring-Wirxel and Kobbelt 2022, <https://doi.org/10.1145/3528223.3530181>).
//!
//! Cost: [`merge`]'s, and [`publish`]'s, linear in what [`merge`] returns.

use core::fmt;

use geom_num::NumError;
use geom_poly::merge::{Merge, MergeError, Outward, merge};
pub use geom_poly::publish::Carrier;
use geom_poly::publish::{PublishError, publish};
use geom_run::WorkMeter;
use geom_topo::{Body, TopoError};

/// Why pieces could not be made into a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebuildError {
    /// The merge refused.
    Merge(MergeError),
    /// The numbers layer refused.
    Num(NumError),
    /// The topology refused.
    Topo(TopoError),
    /// A row past the end of a list: a defect, reported instead of a wrong body.
    Broken,
}

impl fmt::Display for RebuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RebuildError::Merge(e) => write!(f, "{e}"),
            RebuildError::Num(e) => write!(f, "pieces that cannot be rebuilt: {e:?}"),
            RebuildError::Topo(e) => write!(f, "a rebuilt body refused: {e:?}"),
            RebuildError::Broken => f.write_str("a rebuild lost a row"),
        }
    }
}

impl core::error::Error for RebuildError {}

impl From<MergeError> for RebuildError {
    fn from(e: MergeError) -> Self {
        RebuildError::Merge(e)
    }
}

impl From<PublishError> for RebuildError {
    fn from(e: PublishError) -> Self {
        match e {
            PublishError::Num(e) => RebuildError::Num(e),
            PublishError::Topo(e) => RebuildError::Topo(e),
            PublishError::Broken => RebuildError::Broken,
        }
    }
}

impl From<NumError> for RebuildError {
    fn from(e: NumError) -> Self {
        RebuildError::Num(e)
    }
}

impl From<TopoError> for RebuildError {
    fn from(e: TopoError) -> Self {
        RebuildError::Topo(e)
    }
}

/// The body `pieces` bound, every surface from `carriers`, by polygon index.
///
/// # Errors
///
/// A [`RebuildError`]: what [`merge`] refuses, or what the arenas refuse.
pub fn rebuild(
    pieces: &[Outward],
    carriers: &[Carrier],
    mode: Merge,
    meter: &mut WorkMeter<'_>,
) -> Result<Body, RebuildError> {
    let merged = merge(pieces, mode, meter)?;
    Ok(publish(&merged, carriers, meter)?)
}
