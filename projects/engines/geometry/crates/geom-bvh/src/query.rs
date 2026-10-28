//! The box tree's queries: the entries whose boxes meet a box, the pairs of entries whose boxes
//! meet within one tree, and the pairs between two trees.
//!
//! What it does: each query walks the tree from the root with an explicit stack of fixed size,
//! skipping every node whose box misses, and charges one unit of its meter per node, or pair of
//! nodes, it visits. It collects the keys it finds and sorts them.
//!
//! Guarantees:
//!
//! - **The answer of brute force.** A query returns exactly the keys (or key pairs) whose boxes
//!   meet, as testing every entry (or every pair) would, sorted ascending: a pair within one tree
//!   as (smaller key, larger key), a pair between trees as (this tree's key, the other's). So the
//!   answer does not depend on the tree's shape, a refit, a rebuild or the thread count it was
//!   built with.
//! - **No recursion, bounded stacks.** A tree is at most [`MAX_DEPTH`] deep, so a walk of one tree
//!   needs at most `MAX_DEPTH + 1` slots, of two trees at most `2·MAX_DEPTH + 1` (each step
//!   descends one side, pushing two), and of one tree against itself at most `4·MAX_DEPTH + 1`
//!   (a node against itself pushes three). The stacks are arrays of those sizes.
//! - **Cancellable, never partial.** The meter polls the caller's cancellation at fixed charges;
//!   a stopped query returns [`BvhError::Stopped`] and no keys.
//!
//! Method: depth-first traversal of a bounding-volume hierarchy, and the simultaneous traversal
//! of two hierarchies for overlapping pairs, as in G. van den Bergen, "Efficient collision
//! detection of complex deformable models using AABB trees", *Journal of Graphics Tools* 2(4)
//! (1997), 1–13, <https://doi.org/10.1080/10867651.1997.10487480>, with the larger node descended
//! first; answers sorted by key (section 19.9 and chapter 23.7).
//!
//! Cost: O(nodes visited + answers·log answers); memory: the answer.

extern crate alloc;

use alloc::vec::Vec;

use geom_run::WorkMeter;

use crate::bound::Aabb;
use crate::build::MAX_DEPTH;
use crate::error::BvhError;
use crate::tree::{BoxTree, Entry, NodeKind, at};

/// A stack of fixed size.
struct Stack<T, const N: usize> {
    items: [T; N],
    top: usize,
}

impl<T: Copy + Default, const N: usize> Stack<T, N> {
    fn new() -> Self {
        Stack {
            items: [T::default(); N],
            top: 0,
        }
    }

    fn push(&mut self, item: T) -> Result<(), BvhError> {
        *self.items.get_mut(self.top).ok_or(BvhError::Defect)? = item;
        self.top = self.top.checked_add(1).ok_or(BvhError::Defect)?;
        Ok(())
    }

    fn pop(&mut self) -> Option<T> {
        self.top = self.top.checked_sub(1)?;
        self.items.get(self.top).copied()
    }
}

/// Appends `item`, reserving fallibly.
fn put<T>(out: &mut Vec<T>, item: T) -> Result<(), BvhError> {
    if out.len() == out.capacity() {
        out.try_reserve(out.len().max(16))
            .map_err(|_| BvhError::OutOfMemory)?;
    }
    out.push(item);
    Ok(())
}

impl<const D: usize> BoxTree<D> {
    /// The entries of a leaf.
    fn leaf(&self, start: u32, len: u32) -> Result<&[Entry<D>], BvhError> {
        let s = usize::try_from(start).map_err(|_| BvhError::Defect)?;
        let e = s
            .checked_add(usize::try_from(len).map_err(|_| BvhError::Defect)?)
            .ok_or(BvhError::Defect)?;
        self.entries().get(s..e).ok_or(BvhError::Defect)
    }

    /// The keys of the entries whose boxes meet `query`, ascending.
    ///
    /// # Errors
    ///
    /// [`BvhError::Stopped`] if the meter stops; [`BvhError::OutOfMemory`].
    pub fn overlapping(
        &self,
        query: &Aabb<D>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<u64>, BvhError> {
        let mut out = Vec::new();
        if self.nodes().is_empty() {
            return Ok(out);
        }
        let mut stack: Stack<u32, { MAX_DEPTH + 1 }> = Stack::new();
        stack.push(0)?;
        while let Some(n) = stack.pop() {
            meter.charge(1)?;
            let node = at(self.nodes(), n)?;
            if !node.bbox().overlaps(query) {
                continue;
            }
            match node.kind() {
                NodeKind::Leaf { start, len } => {
                    for e in self.leaf(start, len)? {
                        if e.bbox.overlaps(query) {
                            put(&mut out, e.key)?;
                        }
                    }
                }
                NodeKind::Inner { left, right } => {
                    stack.push(right)?;
                    stack.push(left)?;
                }
            }
        }
        out.sort_unstable();
        Ok(out)
    }

    /// Every pair of entries whose boxes meet, as (smaller key, larger key), ascending.
    ///
    /// # Errors
    ///
    /// As [`BoxTree::overlapping`].
    pub fn pairs(&self, meter: &mut WorkMeter<'_>) -> Result<Vec<(u64, u64)>, BvhError> {
        let mut out = Vec::new();
        if self.nodes().is_empty() {
            return Ok(out);
        }
        let mut stack: Stack<(u32, u32), { 4 * MAX_DEPTH + 1 }> = Stack::new();
        stack.push((0, 0))?;
        while let Some((a, b)) = stack.pop() {
            meter.charge(1)?;
            let (na, nb) = (at(self.nodes(), a)?, at(self.nodes(), b)?);
            if a == b {
                match na.kind() {
                    NodeKind::Leaf { start, len } => {
                        let run = self.leaf(start, len)?;
                        for (i, x) in run.iter().enumerate() {
                            for y in run.get(i.saturating_add(1)..).unwrap_or(&[]) {
                                if x.bbox.overlaps(&y.bbox) {
                                    put(&mut out, ordered(x.key, y.key))?;
                                }
                            }
                        }
                    }
                    NodeKind::Inner { left, right } => {
                        stack.push((left, right))?;
                        stack.push((right, right))?;
                        stack.push((left, left))?;
                    }
                }
                continue;
            }
            if !na.bbox().overlaps(&nb.bbox()) {
                continue;
            }
            descend(&other_pair(self, a, b)?, &mut stack, &mut out, ordered)?;
        }
        out.sort_unstable();
        Ok(out)
    }

    /// Every pair of an entry of this tree and an entry of `other` whose boxes meet, as (this
    /// tree's key, the other's), ascending.
    ///
    /// # Errors
    ///
    /// As [`BoxTree::overlapping`].
    pub fn pairs_with(
        &self,
        other: &BoxTree<D>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<(u64, u64)>, BvhError> {
        let mut out = Vec::new();
        if self.nodes().is_empty() || other.nodes().is_empty() {
            return Ok(out);
        }
        let mut stack: Stack<(u32, u32), { 2 * MAX_DEPTH + 1 }> = Stack::new();
        stack.push((0, 0))?;
        while let Some((a, b)) = stack.pop() {
            meter.charge(1)?;
            let (na, nb) = (at(self.nodes(), a)?, at(other.nodes(), b)?);
            if !na.bbox().overlaps(&nb.bbox()) {
                continue;
            }
            let pair = Pair {
                left: (self, a),
                right: (other, b),
            };
            descend(&pair, &mut stack, &mut out, |x, y| (x, y))?;
        }
        out.sort_unstable();
        Ok(out)
    }
}

/// One step of a walk of two nodes whose boxes meet: their leaves' pairs if both are leaves,
/// otherwise the larger inner node (the left one on a tie, or the inner one) split in two.
fn descend<const D: usize, const N: usize>(
    pair: &Pair<'_, D>,
    stack: &mut Stack<(u32, u32), N>,
    out: &mut Vec<(u64, u64)>,
    key: impl Fn(u64, u64) -> (u64, u64),
) -> Result<(), BvhError> {
    let (ta, a) = pair.left;
    let (tb, b) = pair.right;
    let (na, nb) = (at(ta.nodes(), a)?, at(tb.nodes(), b)?);
    match (na.kind(), nb.kind()) {
        (NodeKind::Leaf { start: sa, len: la }, NodeKind::Leaf { start: sb, len: lb }) => {
            for x in ta.leaf(sa, la)? {
                for y in tb.leaf(sb, lb)? {
                    if x.bbox.overlaps(&y.bbox) {
                        put(out, key(x.key, y.key))?;
                    }
                }
            }
        }
        (NodeKind::Inner { left, right }, NodeKind::Leaf { .. }) => {
            stack.push((right, b))?;
            stack.push((left, b))?;
        }
        (NodeKind::Leaf { .. }, NodeKind::Inner { left, right }) => {
            stack.push((a, right))?;
            stack.push((a, left))?;
        }
        (
            NodeKind::Inner { left, right },
            NodeKind::Inner {
                left: bl,
                right: br,
            },
        ) => {
            if na.bbox().area() >= nb.bbox().area() {
                stack.push((right, b))?;
                stack.push((left, b))?;
            } else {
                stack.push((a, br))?;
                stack.push((a, bl))?;
            }
        }
    }
    Ok(())
}

/// Two nodes, each of its own tree.
struct Pair<'t, const D: usize> {
    left: (&'t BoxTree<D>, u32),
    right: (&'t BoxTree<D>, u32),
}

/// Two different nodes of one tree, as a pair.
fn other_pair<const D: usize>(tree: &BoxTree<D>, a: u32, b: u32) -> Result<Pair<'_, D>, BvhError> {
    if a == b {
        return Err(BvhError::Defect);
    }
    Ok(Pair {
        left: (tree, a),
        right: (tree, b),
    })
}

/// A pair of keys, smaller first.
fn ordered(x: u64, y: u64) -> (u64, u64) {
    if x <= y { (x, y) } else { (y, x) }
}
