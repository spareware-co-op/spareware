//! The broad phase: which pairs of entities may touch (11.4 step 1, G242).
//!
//! What it does: [`candidate_pairs`] builds the box tree of `geom-bvh` over boxes the caller has
//! padded by class (exact, certified or Tolerant) and lists every pair whose boxes meet, in key
//! order. [`point_entries`] makes those boxes for points: the box of a point's committed grid
//! point, padded by its bound and by the contact radius, so that two points that can touch have
//! boxes that meet. [`point_pairs`] runs both and returns index pairs for
//! [`crate::merge::merge_stage`].
//!
//! Guarantees:
//!
//! - **No touch is missed.** A pair of points whose separation enclosure holds zero within the
//!   contact radius has overlapping boxes: each box reaches its point's bound plus the contact
//!   radius, whole steps rounded up, and the separation of two committed points is at most the sum
//!   of their bounds when they can be one.
//! - **Pairs in name order.** Keys are the points' indices, every answer is sorted by key, and
//!   the pair list does not depend on the tree's shape, the thread count or the order entries were
//!   given in.
//! - **Cheaper than all pairs.** The tree prunes by box, so a field of points far apart costs a
//!   tree build and a traversal, not n² tests.
//!
//! Method: the one box tree of `geom-bvh` (chapter 19.9 and 23.7), with boxes inflated by class;
//! chapter 11.4 step 1, "Broad phase", for what it is asked to find
//! (`projects/engines/geometry/architecture/11-booleans.md`).
//!
//! Cost: a tree build and a pair traversal, charged to the meter per fixed item. No recursion.

use alloc::vec::Vec;

use geom_bvh::{Aabb, BoxTree, Entry};
use geom_run::{Exec, WorkMeter};

use crate::carrier::PointGeom;
use crate::distance::Carriers;
use crate::rule::{ContactError, EPS_FIT};

/// The boxes of `points`, keyed by index: each point's committed grid point padded by its bound and
/// the contact radius.
///
/// # Errors
///
/// [`ContactError::Num`] when a point cannot be committed; [`ContactError::Broad`] for a box past
/// the box tree's range.
pub fn point_entries(
    carriers: &Carriers,
    points: &[PointGeom],
) -> Result<Vec<Entry<3>>, ContactError> {
    let mut out = Vec::with_capacity(points.len());
    for (key, point) in points.iter().enumerate() {
        let (at, bound) = carriers.commit(point)?;
        let pad = i64::try_from(bound.steps().saturating_add(EPS_FIT.0))
            .map_err(|_| ContactError::Num(geom_num::NumError::BudgetExceeded))?;
        let [x, y, z] = [at.x.get(), at.y.get(), at.z.get()];
        let reach = |c: i64| -> Result<(i64, i64), ContactError> {
            Ok((
                c.checked_sub(pad)
                    .ok_or(ContactError::Num(geom_num::NumError::BudgetExceeded))?,
                c.checked_add(pad)
                    .ok_or(ContactError::Num(geom_num::NumError::BudgetExceeded))?,
            ))
        };
        let ((x0, x1), (y0, y1), (z0, z1)) = (reach(x)?, reach(y)?, reach(z)?);
        out.push(Entry {
            key: u64::try_from(key)
                .map_err(|_| ContactError::Num(geom_num::NumError::BudgetExceeded))?,
            bbox: Aabb::new([x0, y0, z0], [x1, y1, z1])?,
        });
    }
    Ok(out)
}

/// Every pair of entries whose boxes meet, as (smaller key, larger key), ascending.
///
/// # Errors
///
/// [`ContactError::Broad`] when the tree cannot build or answer.
pub fn candidate_pairs(
    entries: &[Entry<3>],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<(u64, u64)>, ContactError> {
    let tree = BoxTree::build(entries, exec, meter)?;
    Ok(tree.pairs(meter)?)
}

/// The index pairs of `points` that may touch, in index order.
///
/// # Errors
///
/// As [`point_entries`] and [`candidate_pairs`].
pub fn point_pairs(
    carriers: &Carriers,
    points: &[PointGeom],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<(usize, usize)>, ContactError> {
    let entries = point_entries(carriers, points)?;
    let mut out = Vec::new();
    for (a, b) in candidate_pairs(&entries, exec, meter)? {
        let both = (usize::try_from(a), usize::try_from(b));
        if let (Ok(a), Ok(b)) = both {
            out.push((a, b));
        }
    }
    Ok(out)
}
