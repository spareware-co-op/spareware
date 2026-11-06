//! The canonical rebuild: merged pieces published as a body (11.3, step 5).
//!
//! What it does: [`rebuild`] takes the pieces a selection keeps, has [`merge_with`] make them
//! faces, edges and shells, and has [`publish`] write those into a body; [`wire`] writes a
//! section's curves. The writing itself is `geom-poly`'s, shared with `geom-mesh`'s faceted
//! B-rep: here its refusals, and the naming that only booleans do, become a [`RebuildError`].
//!
//! Guarantees:
//!
//! - **The same bytes however the boxes fell.** Every row comes in the order [`merge_with`]
//!   numbers the shape, which depends on the shape alone (11.3; tested).
//! - **Refused, never guessed.** What [`merge_with`] and [`publish`] refuse is a [`RebuildError`].
//!
//! Method: chapter 11.3's rebuild (`projects/engines/geometry/architecture/11-booleans.md`), after
//! EMBER (Trettner, Nehring-Wirxel and Kobbelt 2022, <https://doi.org/10.1145/3528223.3530181>).
//!
//! Cost: [`merge_with`]'s, and [`publish`]'s, linear in what [`merge_with`] returns.

use core::fmt;

use geom_num::NumError;
use geom_poly::merge::{Curves, Merge, MergeError, Merged, Outward, Rules, merge_with};
pub use geom_poly::publish::Carrier;
use geom_poly::publish::PublishError;
use geom_run::WorkMeter;
use geom_topo::order::Names;
use geom_topo::{Body, TopoError};

use crate::names::NamingError;

/// Why pieces could not be made into a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebuildError {
    /// The merge refused.
    Merge(MergeError),
    /// The numbers layer refused.
    Num(NumError),
    /// The topology refused.
    Topo(TopoError),
    /// A name could not be formed.
    Naming(NamingError),
    /// A row past the end of a list: a defect, reported instead of a wrong body.
    Broken,
}

impl fmt::Display for RebuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RebuildError::Merge(e) => write!(f, "{e}"),
            RebuildError::Num(e) => write!(f, "pieces that cannot be rebuilt: {e:?}"),
            RebuildError::Topo(e) => write!(f, "a rebuilt body refused: {e:?}"),
            RebuildError::Naming(e) => write!(f, "{e}"),
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
/// A [`RebuildError`]: what [`merge_with`] refuses, or what the arenas refuse.
pub fn rebuild(
    pieces: &[Outward],
    carriers: &[Carrier],
    mode: Merge,
    meter: &mut WorkMeter<'_>,
) -> Result<Body, RebuildError> {
    rebuild_with(pieces, carriers, mode, Rules::default(), meter)
}

/// [`rebuild`], under `rules`: an open merge gives a sheet.
///
/// # Errors
///
/// As [`rebuild`].
pub fn rebuild_with(
    pieces: &[Outward],
    carriers: &[Carrier],
    mode: Merge,
    rules: Rules<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Body, RebuildError> {
    let merged = merge_with(pieces, mode, rules, meter)?;
    publish(&merged, carriers, rules.open, None, meter)
}

/// `merged` written into arenas and published, in the order of `names` if it has any: an open
/// merge is a sheet, and a closed one a solid unless some edge has more than two fins.
///
/// # Errors
///
/// A [`RebuildError`]: what the arenas refuse.
pub fn publish(
    merged: &Merged,
    carriers: &[Carrier],
    open: bool,
    names: Option<&Names>,
    meter: &mut WorkMeter<'_>,
) -> Result<Body, RebuildError> {
    Ok(geom_poly::publish::publish_with(
        merged, carriers, open, names, meter,
    )?)
}

/// Whether some vertex's fins in one shell go round it in more than one fan: the shell's surface
/// pinches there. Each edge has two fins. A fan is walked from a fin leaving the vertex to the fin
/// leaving it after the one before it in its ring, across their edge, until it comes back.
///
/// Cost: one step per fin.
fn pinched(merged: &Merged) -> bool {
    let fins = &merged.fins;
    let mut before = alloc::vec![None; fins.len()];
    for ring in merged.faces.iter().flat_map(|face| &face.rings) {
        for (fin, next) in ring.iter().zip(ring.iter().cycle().skip(1)) {
            if let Some(slot) = before.get_mut(*next) {
                *slot = Some(*fin);
            }
        }
    }
    let across = |fin: usize| {
        let edge = merged.edges.get(fins.get(fin)?.edge)?;
        edge.fins.iter().copied().find(|other| *other != fin)
    };
    let mut seen = alloc::vec![false; fins.len()];
    let mut walked = BTreeSet::new();
    for (start, fin) in fins.iter().enumerate() {
        if seen.get(start).copied().unwrap_or(true) {
            continue;
        }
        let mut at = Some(start);
        while let Some(slot) = at.and_then(|f| seen.get_mut(f)).filter(|done| !**done) {
            *slot = true;
            at = at
                .and_then(|f| before.get(f).copied().flatten())
                .and_then(across);
        }
        let shell = merged.faces.get(fin.face).map(|face| face.shell);
        if !walked.insert((shell, fin.from)) {
            return true;
        }
    }
    false
}

/// The wire body of `curves`: edges with no fins, each vertex a grid point or its own meeting.
///
/// # Errors
///
/// A [`RebuildError`]: what the arenas refuse.
pub fn wire(
    curves: &Curves,
    names: Option<&Names>,
    meter: &mut WorkMeter<'_>,
) -> Result<Body, RebuildError> {
    Ok(geom_poly::publish::wire(curves, names, meter)?)
}
