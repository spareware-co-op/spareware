//! The R-tree behind the index: the one file that names the `rstar` crate.
//!
//! What it is: a thin wrapper that stores [`Entry`] values in an R-tree and answers three raw
//! questions (boxes that meet a box, boxes inside a box, entries nearest a point). It promises
//! nothing about order: `lib.rs` sorts every answer by id, so the order the tree walks never
//! reaches a caller. It depends on `rstar` (MIT OR Apache-2.0) and on this crate's own types.
//!
//! Replacing the tree (a different crate, or a hand-made grid) changes this file alone, and the
//! tests, which are written against [`crate::SpatialIndex`], judge the replacement unchanged.
//!
//! Why the walk is the same on every machine and run: `rstar` 0.12 builds and searches with no
//! parallelism, no hash map and no random numbers, and every coordinate here is a whole number of
//! millimetres under 2^40, so each is held exactly in an `f64` (53 bits) and the comparisons that
//! decide a query are exact. The `f64` arithmetic the tree does for its own balancing only picks
//! the tree's shape; it is IEEE arithmetic with no fused operations, so the same inserts in the
//! same order give the same tree everywhere, and the answers do not depend on the shape at all.

use crate::{Entry, MAX_COORDINATE};
use construct_model::geometry::Bounds;
use rstar::{AABB, PointDistance, RTree, RTreeObject};

/// The key an [`Entry`] is looked up by in the tree: its box.
impl RTreeObject for Entry {
    type Envelope = AABB<[f64; 3]>;

    fn envelope(&self) -> Self::Envelope {
        to_aabb(&self.bounds)
    }
}

/// Distance to a point is the tree's own distance to the entry's box.
impl PointDistance for Entry {
    fn distance_2(&self, point: &[f64; 3]) -> f64 {
        self.envelope().distance_2(point)
    }
}

/// A box in the tree's number form.
///
/// Every coordinate is within `MAX_COORDINATE` (checked before anything reaches the tree), so the
/// conversion is exact.
#[allow(clippy::cast_precision_loss)]
fn to_aabb(bounds: &Bounds) -> AABB<[f64; 3]> {
    debug_assert!(
        bounds
            .min
            .iter()
            .chain(&bounds.max)
            .all(|c| c.abs() <= MAX_COORDINATE)
    );
    let low = bounds.min.map(|c| c as f64);
    let high = bounds.max.map(|c| c as f64);
    AABB::from_corners(low, high)
}

/// A point in the tree's number form, exact for the same reason.
#[allow(clippy::cast_precision_loss)]
fn to_point(point: [i64; 3]) -> [f64; 3] {
    point.map(|c| c as f64)
}

/// The tree.
#[derive(Debug, Default, Clone)]
pub(crate) struct Tree {
    inner: RTree<Entry>,
}

impl Tree {
    /// A tree holding exactly `entries`, bulk loaded.
    pub(crate) fn bulk(entries: Vec<Entry>) -> Self {
        Self {
            inner: RTree::bulk_load(entries),
        }
    }

    /// Adds an entry. The caller has made sure its id is not already in the tree.
    pub(crate) fn insert(&mut self, entry: Entry) {
        self.inner.insert(entry);
    }

    /// Takes out the entry equal to `entry`, if the tree holds one.
    pub(crate) fn remove(&mut self, entry: &Entry) {
        self.inner.remove(entry);
    }

    /// Every entry whose box meets `query`, touching included, in no defined order.
    pub(crate) fn meeting<'a>(&'a self, query: &Bounds) -> impl Iterator<Item = &'a Entry> {
        self.inner.locate_in_envelope_intersecting(&to_aabb(query))
    }

    /// Every entry whose box lies inside `query`, touching included, in no defined order.
    pub(crate) fn inside<'a>(&'a self, query: &Bounds) -> impl Iterator<Item = &'a Entry> {
        self.inner.locate_in_envelope(&to_aabb(query))
    }

    /// Entries by rising distance to `point`, each with the tree's own squared distance as a
    /// float. The float is exact up to rounding only (squares reach 2^82): it is a bound for
    /// deciding when to stop reading, never the distance that ranks an answer.
    pub(crate) fn by_distance(&self, point: [i64; 3]) -> impl Iterator<Item = (&Entry, f64)> {
        let at = to_point(point);
        self.inner.nearest_neighbor_iter_with_distance_2(&at)
    }
}
