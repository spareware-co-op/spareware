//! The instance tree of one assembly: the kernel's one box tree over its records' boxes, whose
//! leaves hold only record indices (19.12; piece G295).
//!
//! What it does: [`InstanceTree::build`] gives `geom-bvh`'s [`BoxTree`] one entry per record, its
//! key the record's index and its box the record's, builds the tree, and keeps only what a query
//! walks: the tree's nodes, as `geom-bvh` built them, and the record indices in leaf order. The
//! boxes the entries carried are dropped, since every record holds its own (19.12: "its integer
//! box, held once"). [`InstanceTree::overlapping`] walks the nodes with an explicit stack and tests
//! each leaf's records by their own boxes.
//!
//! Guarantees:
//!
//! - **The answer of brute force.** [`InstanceTree::overlapping`] returns exactly the indices of
//!   the records whose boxes meet the region, ascending, as testing every record would: never the
//!   tree's shape, a pool's thread count or the records' order decides it.
//! - **The kernel's one tree.** The nodes are `geom-bvh`'s, built by its binned surface-area
//!   heuristic (19.9), so a tree is a function of its records' boxes alone: the same nodes for any
//!   pool, thread count and chunking.
//! - **Four bytes a record.** Beside the nodes, a record costs the tree one `u32`; the 1,000,000
//!   instance benchmark (`benches/instance.rs`) measures the total.
//! - **No recursion.** A walk holds at most `MAX_DEPTH + 1` nodes on its stack, as `geom-bvh`'s
//!   walks do; the meter is charged one unit a node and polls the caller's cancellation.
//!
//! Method: the two-level tree of chapter 19.12, its top level here
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`), on the box tree of
//! section 19.9, built by the binned surface-area heuristic of I. Wald, "On fast construction of
//! SAH-based bounding volume hierarchies", IEEE Symposium on Interactive Ray Tracing (2007),
//! <https://doi.org/10.1109/RT.2007.4342588>, and walked depth first as in G. van den Bergen,
//! "Efficient collision detection of complex deformable models using AABB trees", *Journal of
//! Graphics Tools* 2(4) (1997), <https://doi.org/10.1080/10867651.1997.10487480>.
//!
//! Cost: building as `geom-bvh`'s build over n entries, plus O(n) to copy the nodes and the leaf
//! order out; while it builds, the entries and `geom-bvh`'s working copies (about 56 bytes a
//! record each). Kept: one `u32` a record and the nodes. A query: O(nodes visited + answers·log
//! answers). Recursion: none.

use alloc::vec::Vec;

use geom_bvh::{Aabb, BoxTree, Entry, MAX_DEPTH, Node, NodeKind};
use geom_math::Box3;
use geom_num::grid::P3;
use geom_run::{Exec, LogicalSize, WorkMeter};

use super::error::InstanceError;
use super::record::Record;
use super::region::Region;

/// A record index in a leaf, for the logical-memory budget (23.10).
pub struct LeafIndex;

impl LogicalSize for LeafIndex {
    /// One `u32`.
    const LOGICAL_BYTES: u64 = 4;
}

/// A node of the tree, for the logical-memory budget (23.10).
pub struct TreeNode;

impl LogicalSize for TreeNode {
    /// Its box (six `i64`s) and two `u32` links.
    const LOGICAL_BYTES: u64 = 56;
}

/// The instance tree of one assembly (module documentation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceTree {
    /// `geom-bvh`'s nodes: the root is node 0, and an empty tree has none.
    nodes: Vec<Node<3>>,
    /// The record indices in leaf order: a leaf's `start..start + len` are its records.
    leaves: Vec<u32>,
}

/// `b` as the box tree's box.
pub(crate) fn aabb(b: &Box3) -> Result<Aabb<3>, InstanceError> {
    let (lo, hi) = (b.min(), b.max());
    Ok(Aabb::new(
        [lo.x.get(), lo.y.get(), lo.z.get()],
        [hi.x.get(), hi.y.get(), hi.z.get()],
    )?)
}

/// The box tree's box `b` as a grid box.
///
/// # Errors
///
/// [`InstanceError::Num`] for a corner beyond the grid's range, which the box tree's limit allows.
pub(crate) fn grid_box(b: &Aabb<3>) -> Result<Box3, InstanceError> {
    let ([lx, ly, lz], [hx, hy, hz]) = (b.lo(), b.hi());
    Ok(Box3::new(P3::new(lx, ly, lz)?, P3::new(hx, hy, hz)?)?)
}

/// Appends `item`, reserving fallibly.
pub(crate) fn put<T>(out: &mut Vec<T>, item: T) -> Result<(), InstanceError> {
    out.try_reserve(1).map_err(|_| InstanceError::OutOfMemory)?;
    out.push(item);
    Ok(())
}

impl InstanceTree {
    /// The tree over `records`, record i keyed i (module documentation). Charges the meter the
    /// build's work and the tree's logical bytes.
    ///
    /// # Errors
    ///
    /// [`InstanceError::TooManyRecords`] past `u32::MAX` records; [`InstanceError::Tree`] for a box
    /// the box tree refuses; [`InstanceError::Stopped`] if the meter stops;
    /// [`InstanceError::OutOfMemory`].
    pub fn build<R: Record>(
        records: &[R],
        exec: &Exec<'_>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<InstanceTree, InstanceError> {
        let count = u32::try_from(records.len()).map_err(|_| InstanceError::TooManyRecords)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(records.len())
            .map_err(|_| InstanceError::OutOfMemory)?;
        for (key, record) in (0..count).zip(records) {
            entries.push(Entry {
                key: u64::from(key),
                bbox: aabb(record.bound())?,
            });
        }
        // Keys 0, 1, 2, ... are already in order, so the box tree skips its sort.
        let tree = BoxTree::build(&entries, exec, meter)?;
        drop(entries);
        meter.reserve_for::<LeafIndex>(u64::from(count))?;
        meter.reserve_for::<TreeNode>(
            u64::try_from(tree.nodes().len()).map_err(|_| InstanceError::Defect)?,
        )?;
        let mut leaves = Vec::new();
        leaves
            .try_reserve_exact(tree.len())
            .map_err(|_| InstanceError::OutOfMemory)?;
        for e in tree.entries() {
            leaves.push(u32::try_from(e.key).map_err(|_| InstanceError::Defect)?);
        }
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(tree.nodes().len())
            .map_err(|_| InstanceError::OutOfMemory)?;
        nodes.extend_from_slice(tree.nodes());
        Ok(InstanceTree { nodes, leaves })
    }

    /// How many records the tree indexes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    /// Whether the tree indexes no record.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    /// The nodes, as `geom-bvh` built them: the root is node 0.
    #[must_use]
    pub fn nodes(&self) -> &[Node<3>] {
        &self.nodes
    }

    /// The record indices in leaf order.
    #[must_use]
    pub fn leaves(&self) -> &[u32] {
        &self.leaves
    }

    /// The box of every record, or `None` for no records.
    ///
    /// # Errors
    ///
    /// [`InstanceError::Num`] only if the root's box leaves the grid's range, which records' boxes,
    /// grid boxes, rule out.
    pub fn bound(&self) -> Result<Option<Box3>, InstanceError> {
        self.nodes
            .first()
            .map(|root| grid_box(&root.bbox()))
            .transpose()
    }

    /// The heap bytes the tree holds: its nodes' and leaves' capacities, at this target's sizes.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.nodes
            .capacity()
            .saturating_mul(size_of::<Node<3>>())
            .saturating_add(self.leaves.capacity().saturating_mul(size_of::<u32>()))
    }

    /// The indices of the records whose boxes meet `region`, ascending. `records` are the records
    /// the tree was built over, in the same order.
    ///
    /// # Errors
    ///
    /// [`InstanceError::Stale`] if `records` is not as long as the tree;
    /// [`InstanceError::Stopped`] if the meter stops; [`InstanceError::OutOfMemory`].
    pub fn overlapping<R: Record>(
        &self,
        records: &[R],
        region: &Box3,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<u32>, InstanceError> {
        self.meeting(records, &Region::from_box(region), meter)
    }

    /// As [`InstanceTree::overlapping`], for a region with rational corners.
    ///
    /// # Errors
    ///
    /// As [`InstanceTree::overlapping`].
    pub fn meeting<R: Record>(
        &self,
        records: &[R],
        region: &Region,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<u32>, InstanceError> {
        if records.len() != self.leaves.len() {
            return Err(InstanceError::Stale(
                u32::try_from(records.len()).unwrap_or(u32::MAX),
            ));
        }
        let mut out = Vec::new();
        walk(
            &self.nodes,
            |b| region.meets_aabb(b),
            |run| {
                for &r in self.leaves.get(run).ok_or(InstanceError::Defect)? {
                    let record = records
                        .get(usize::try_from(r).map_err(|_| InstanceError::Defect)?)
                        .ok_or(InstanceError::Defect)?;
                    if region.meets_box(record.bound()) {
                        put(&mut out, r)?;
                    }
                }
                Ok(())
            },
            meter,
        )?;
        out.sort_unstable();
        Ok(out)
    }
}

/// The keys of the parts of `tree` whose boxes meet `region`, ascending: the box tree's own
/// query, for a region with rational corners.
///
/// # Errors
///
/// [`InstanceError::Stopped`] if the meter stops; [`InstanceError::OutOfMemory`].
pub(crate) fn parts_meeting(
    tree: &BoxTree<3>,
    region: &Region,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<u64>, InstanceError> {
    let mut out = Vec::new();
    walk(
        tree.nodes(),
        |b| region.meets_aabb(b),
        |run| {
            for e in tree.entries().get(run).ok_or(InstanceError::Defect)? {
                if region.meets_aabb(&e.bbox) {
                    put(&mut out, e.key)?;
                }
            }
            Ok(())
        },
        meter,
    )?;
    out.sort_unstable();
    Ok(out)
}

/// Walks `nodes` from the root, depth first on an explicit stack, into every node whose box
/// `keep` accepts, and gives each such leaf's run of positions to `leaf`. Charges the meter one
/// unit a node.
fn walk(
    nodes: &[Node<3>],
    keep: impl Fn(&Aabb<3>) -> bool,
    mut leaf: impl FnMut(core::ops::Range<usize>) -> Result<(), InstanceError>,
    meter: &mut WorkMeter<'_>,
) -> Result<(), InstanceError> {
    if nodes.is_empty() {
        return Ok(());
    }
    let mut stack: Vec<u32> = Vec::new();
    stack
        .try_reserve(MAX_DEPTH)
        .map_err(|_| InstanceError::OutOfMemory)?;
    stack.push(0);
    while let Some(n) = stack.pop() {
        meter.charge(1)?;
        let node = nodes
            .get(usize::try_from(n).map_err(|_| InstanceError::Defect)?)
            .ok_or(InstanceError::Defect)?;
        if !keep(&node.bbox()) {
            continue;
        }
        match node.kind() {
            NodeKind::Leaf { start, len } => {
                let s = usize::try_from(start).map_err(|_| InstanceError::Defect)?;
                let e = s
                    .checked_add(usize::try_from(len).map_err(|_| InstanceError::Defect)?)
                    .ok_or(InstanceError::Defect)?;
                leaf(s..e)?;
            }
            NodeKind::Inner { left, right } => {
                put(&mut stack, right)?;
                put(&mut stack, left)?;
            }
        }
    }
    Ok(())
}
