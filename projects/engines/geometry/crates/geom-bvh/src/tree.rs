//! The box tree: its entries, its nodes, how it is built, refitted and rebuilt, and its canonical
//! bytes.
//!
//! What it does: [`BoxTree::build`] sorts the entries by key, refuses a repeated key, and builds
//! the nodes ([`crate::build`]). The entries are then stored in leaf order, each leaf a range of
//! them, beside an index from key to position. [`BoxTree::refit`] gives entries new boxes and
//! passes the change up to every node. When the tree's cost ([`BoxTree::cost`], the
//! surface-area heuristic summed over its nodes) passes [`REBUILD_FACTOR`] times the cost it was
//! built with, the refit rebuilds the tree from its entries instead. [`BoxTree::to_bytes`]
//! writes the tree in a canonical form, for goldens and caches.
//!
//! Guarantees:
//!
//! - **A function of its entries.** A tree is built from the set of its entries alone: their
//!   input order, the pool, the thread count and the chunking change nothing, down to the last
//!   byte of [`BoxTree::to_bytes`]. A rebuild equals a fresh build of the same entries.
//! - **Every node holds what is below it.** A node's box is the union of its children's, a leaf's
//!   the union of its entries', after a build and after a refit.
//! - **All or nothing.** A refit checks every update before it changes anything, so a refused
//!   refit, or one stopped by its meter while rebuilding, leaves the tree as it was.
//! - **Children after parents.** A node's children have larger indices than it has, so one pass
//!   from the last node to the first refits the tree.
//!
//! Method: section 19.9 of the kernel architecture ("Refit or rebuild"; "Answers do not depend on
//! the tree"), with the tree's cost the surface-area heuristic of I. Wald, "On fast construction of
//! SAH-based bounding volume hierarchies", IEEE Symposium on Interactive Ray Tracing (2007),
//! <https://doi.org/10.1109/RT.2007.4342588>, one unit per inner node and one per entry in a leaf.
//!
//! Cost: building as [`crate::build`], plus a stable sort by key; a refit O(updates·log n + nodes);
//! the bytes O(entries + nodes). Memory: the entries, the nodes and the key index.

extern crate alloc;

use alloc::vec::Vec;

use geom_run::{ChunkPlan, Exec, Phase, RunError, WorkMeter};

use crate::bound::Aabb;
use crate::build::{self, MAX_DEPTH};
use crate::error::BvhError;

/// The factor of its built cost past which a refit rebuilds the tree.
pub const REBUILD_FACTOR: u128 = 2;

/// The chunk size for sorting entries by key.
const SORT_PLAN: ChunkPlan = ChunkPlan::fixed::<4096>();

/// The phase that sorts entries by key.
const SORT: Phase = Phase::new("box tree: sort by key");

/// An entry of the tree: a persistent key and its box.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Entry<const D: usize> {
    /// The entry's key: a persistent name, or an arena index inside one operation. Keys are
    /// distinct, break every tie, and order every answer.
    pub key: u64,
    /// The entry's box.
    pub bbox: Aabb<D>,
}

/// What a node is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// A leaf: the entries at positions `start..start + len` of [`BoxTree::entries`].
    Leaf {
        /// The first entry's position.
        start: u32,
        /// How many entries.
        len: u32,
    },
    /// An inner node: the indices of its two children in [`BoxTree::nodes`].
    Inner {
        /// The left child.
        left: u32,
        /// The right child.
        right: u32,
    },
}

/// A node of the tree: a box holding everything below it, and what the node is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Node<const D: usize> {
    bbox: Aabb<D>,
    kind: NodeKind,
}

impl<const D: usize> Node<D> {
    pub(crate) const fn new(bbox: Aabb<D>, kind: NodeKind) -> Node<D> {
        Node { bbox, kind }
    }

    /// The node's box.
    #[must_use]
    pub const fn bbox(&self) -> Aabb<D> {
        self.bbox
    }

    /// What the node is.
    #[must_use]
    pub const fn kind(&self) -> NodeKind {
        self.kind
    }
}

/// What a refit did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Refitted {
    /// The boxes changed and the nodes followed them.
    Refit,
    /// The cost passed its limit, so the tree was built again from its entries.
    Rebuilt,
}

/// The one tree of integer boxes (module documentation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoxTree<const D: usize> {
    entries: Vec<Entry<D>>,
    nodes: Vec<Node<D>>,
    /// `by_key[i]` is the position in `entries` of the entry with the i-th smallest key.
    by_key: Vec<u32>,
    built_cost: u128,
}

pub(crate) fn at<T: Copy>(items: &[T], i: u32) -> Result<T, BvhError> {
    items
        .get(usize::try_from(i).map_err(|_| BvhError::Defect)?)
        .copied()
        .ok_or(BvhError::Defect)
}

/// `entries` sorted by key, with a repeated key refused.
fn sorted_by_key<const D: usize>(
    entries: &[Entry<D>],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Entry<D>>, BvhError> {
    if u32::try_from(entries.len()).is_err() || entries.len() == usize::MAX {
        return Err(BvhError::TooManyEntries);
    }
    let order = exec
        .sort_stable(meter, SORT, SORT_PLAN, entries, |e: &Entry<D>| e.key)
        .map_err(|e: RunError<BvhError>| BvhError::from(e))?;
    let mut sorted = Vec::new();
    sorted
        .try_reserve_exact(entries.len())
        .map_err(|_| BvhError::OutOfMemory)?;
    for i in order {
        sorted.push(*entries.get(i).ok_or(BvhError::Defect)?);
    }
    for pair in sorted.windows(2) {
        if let [a, b] = pair
            && a.key == b.key
        {
            return Err(BvhError::DuplicateKey(a.key));
        }
    }
    Ok(sorted)
}

impl<const D: usize> BoxTree<D> {
    /// The tree over `entries` (module documentation).
    ///
    /// # Errors
    ///
    /// [`BvhError::TooManyEntries`] for more than `u32::MAX` entries; [`BvhError::DuplicateKey`]
    /// for the smallest repeated key; [`BvhError::Stopped`] or [`BvhError::BudgetExceeded`] if the
    /// meter stops the build; [`BvhError::OutOfMemory`].
    pub fn build(
        entries: &[Entry<D>],
        exec: &Exec<'_>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<BoxTree<D>, BvhError> {
        let sorted = sorted_by_key(entries, exec, meter)?;
        let (nodes, order) = build::build(&sorted, exec, meter)?;
        let mut leaf_order = Vec::new();
        leaf_order
            .try_reserve_exact(sorted.len())
            .map_err(|_| BvhError::OutOfMemory)?;
        let mut by_key = Vec::new();
        by_key
            .try_reserve_exact(sorted.len())
            .map_err(|_| BvhError::OutOfMemory)?;
        by_key.resize(sorted.len(), 0);
        for (position, &id) in order.iter().enumerate() {
            leaf_order.push(at(&sorted, id)?);
            *by_key
                .get_mut(usize::try_from(id).map_err(|_| BvhError::Defect)?)
                .ok_or(BvhError::Defect)? =
                u32::try_from(position).map_err(|_| BvhError::Defect)?;
        }
        let mut tree = BoxTree {
            entries: leaf_order,
            nodes,
            by_key,
            built_cost: 0,
        };
        tree.built_cost = tree.cost();
        Ok(tree)
    }

    /// The entries, in leaf order.
    #[must_use]
    pub fn entries(&self) -> &[Entry<D>] {
        &self.entries
    }

    /// The nodes; the root is node 0, and an empty tree has none.
    #[must_use]
    pub fn nodes(&self) -> &[Node<D>] {
        &self.nodes
    }

    /// How many entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the tree holds no entry.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The tree's cost: Σ area(node) over inner nodes plus Σ area(leaf)·entries over leaves, the
    /// surface-area heuristic with one unit per node visited and per entry tested; saturating at
    /// `u128::MAX`, which is past every limit.
    #[must_use]
    pub fn cost(&self) -> u128 {
        self.nodes.iter().fold(0u128, |sum, node| {
            let weight = match node.kind {
                NodeKind::Leaf { len, .. } => u128::from(len),
                NodeKind::Inner { .. } => 1,
            };
            sum.saturating_add(node.bbox.area().saturating_mul(weight))
        })
    }

    /// The cost the tree had when it was built.
    #[must_use]
    pub fn built_cost(&self) -> u128 {
        self.built_cost
    }

    /// The depth of the deepest node; 0 for a tree of one node or none.
    ///
    /// # Errors
    ///
    /// [`BvhError::Defect`] if the nodes are not a tree of depth at most
    /// [`build::MAX_DEPTH`], which construction rules out.
    pub fn depth(&self) -> Result<usize, BvhError> {
        if self.nodes.is_empty() {
            return Ok(0);
        }
        let mut deepest = 0;
        let mut stack = [(0u32, 0usize); MAX_DEPTH + 2];
        let mut top = 1usize;
        while let Some(t) = top.checked_sub(1) {
            top = t;
            let (node, depth) = *stack.get(t).ok_or(BvhError::Defect)?;
            if depth > deepest {
                deepest = depth;
            }
            if let NodeKind::Inner { left, right } = at(&self.nodes, node)?.kind {
                let next = depth.checked_add(1).ok_or(BvhError::Defect)?;
                for child in [right, left] {
                    *stack.get_mut(top).ok_or(BvhError::Defect)? = (child, next);
                    top = top.checked_add(1).ok_or(BvhError::Defect)?;
                }
            }
        }
        Ok(deepest)
    }

    /// Gives each update's key its new box, passes the change up the tree, and rebuilds the tree
    /// if its cost then passes [`REBUILD_FACTOR`] times its built cost.
    ///
    /// # Errors
    ///
    /// [`BvhError::UnknownKey`] for an update whose key the tree does not hold;
    /// [`BvhError::DuplicateKey`] for a key updated twice; the errors of [`BoxTree::build`] when
    /// rebuilding. On any error the tree is unchanged.
    pub fn refit(
        &mut self,
        updates: &[Entry<D>],
        exec: &Exec<'_>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Refitted, BvhError> {
        let sorted = sorted_by_key(updates, exec, meter)?;
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(sorted.len())
            .map_err(|_| BvhError::OutOfMemory)?;
        for u in &sorted {
            let i = self
                .by_key
                .binary_search_by(|&p| at(&self.entries, p).map_or(0, |e| e.key).cmp(&u.key))
                .map_err(|_| BvhError::UnknownKey(u.key))?;
            positions.push(at(
                &self.by_key,
                u32::try_from(i).map_err(|_| BvhError::Defect)?,
            )?);
        }
        meter.charge(
            u64::try_from(self.nodes.len().saturating_add(sorted.len()))
                .map_err(|_| BvhError::Defect)?,
        )?;
        let mut next = self.clone();
        for (u, p) in sorted.iter().zip(positions) {
            let slot = next
                .entries
                .get_mut(usize::try_from(p).map_err(|_| BvhError::Defect)?)
                .ok_or(BvhError::Defect)?;
            slot.bbox = u.bbox;
        }
        next.refit_nodes()?;
        if next.cost() > self.built_cost.saturating_mul(REBUILD_FACTOR) {
            *self = BoxTree::build(&next.entries, exec, meter)?;
            return Ok(Refitted::Rebuilt);
        }
        *self = next;
        Ok(Refitted::Refit)
    }

    /// Every node's box from its children's or its entries', last node first.
    fn refit_nodes(&mut self) -> Result<(), BvhError> {
        for i in (0..self.nodes.len()).rev() {
            let node = *self.nodes.get(i).ok_or(BvhError::Defect)?;
            let bbox = match node.kind {
                NodeKind::Leaf { start, len } => {
                    let s = usize::try_from(start).map_err(|_| BvhError::Defect)?;
                    let e = s
                        .checked_add(usize::try_from(len).map_err(|_| BvhError::Defect)?)
                        .ok_or(BvhError::Defect)?;
                    let run = self.entries.get(s..e).ok_or(BvhError::Defect)?;
                    let first = run.first().ok_or(BvhError::Defect)?.bbox;
                    run.iter().fold(first, |acc, entry| acc.union(&entry.bbox))
                }
                NodeKind::Inner { left, right } => at(&self.nodes, left)?
                    .bbox
                    .union(&at(&self.nodes, right)?.bbox),
            };
            *self.nodes.get_mut(i).ok_or(BvhError::Defect)? = Node::new(bbox, node.kind);
        }
        Ok(())
    }

    /// The tree's canonical bytes: `SKBVH1`, D as one byte, the entry and node counts as
    /// little-endian `u32`s, then each entry in leaf order (its key as a little-endian `u64`, its
    /// box's lower and upper corners as little-endian `i64`s) and each node in index order (its box
    /// as for an entry, then 0 and its leaf's start and length, or 1 and its children, one byte and
    /// two little-endian `u32`s).
    ///
    /// # Errors
    ///
    /// [`BvhError::OutOfMemory`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, BvhError> {
        let per_box = D.saturating_mul(16);
        let size = 6usize
            .saturating_add(9)
            .saturating_add(self.entries.len().saturating_mul(per_box.saturating_add(8)))
            .saturating_add(self.nodes.len().saturating_mul(per_box.saturating_add(9)));
        let mut out = Vec::new();
        out.try_reserve_exact(size)
            .map_err(|_| BvhError::OutOfMemory)?;
        out.extend_from_slice(b"SKBVH1");
        out.push(u8::try_from(D).map_err(|_| BvhError::Defect)?);
        for n in [self.entries.len(), self.nodes.len()] {
            out.extend_from_slice(
                &u32::try_from(n)
                    .map_err(|_| BvhError::Defect)?
                    .to_le_bytes(),
            );
        }
        let put_box = |out: &mut Vec<u8>, b: &Aabb<D>| {
            for c in b.lo().iter().chain(b.hi().iter()) {
                out.extend_from_slice(&c.to_le_bytes());
            }
        };
        for e in &self.entries {
            out.extend_from_slice(&e.key.to_le_bytes());
            put_box(&mut out, &e.bbox);
        }
        for n in &self.nodes {
            put_box(&mut out, &n.bbox);
            let (tag, a, b) = match n.kind {
                NodeKind::Leaf { start, len } => (0u8, start, len),
                NodeKind::Inner { left, right } => (1u8, left, right),
            };
            out.push(tag);
            out.extend_from_slice(&a.to_le_bytes());
            out.extend_from_slice(&b.to_le_bytes());
        }
        Ok(out)
    }
}
