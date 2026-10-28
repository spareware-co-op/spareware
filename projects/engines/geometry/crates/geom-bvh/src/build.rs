//! The box tree's construction: top-down, by the binned surface-area heuristic with whole-number
//! costs, in parallel only through `geom-run`'s helper.
//!
//! What it does: [`build`] takes the entries sorted by key and returns the tree's nodes and the
//! order of the entries along its leaves. Each node's entries are split by the plane, among
//! b − 1 planes per axis between b bins of their boxes' centres (b = min(entries, [`BINS`])), that
//! minimises
//! area(left)·count(left) + area(right)·count(right), or kept as a leaf once there are at most
//! [`LEAF_MAX`]. The top of the tree is built on the calling thread until every unbuilt range holds
//! at most [`GRAIN`] entries; those subtrees are then built as one phase of the helper, one task
//! each, and spliced in task order.
//!
//! Guarantees:
//!
//! - **One tree for one set of entries.** Every split is a pure function of the range's entries:
//!   the bins and the costs are whole numbers ([`crate::bound`] bounds them), ties between planes
//!   go to the lower axis and then the lower plane, and the partition is stable, so each range
//!   keeps its entries in key order. Which ranges become subtree tasks depends on their sizes
//!   alone. So the nodes, their order and the entries' order are the same for any thread count,
//!   pool and chunking, and for any input order of the same entries.
//! - **Depth at most [`MAX_DEPTH`].** From depth [`MEDIAN_DEPTH`] = 32 on, every split is at the
//!   median in key order. A range there holds fewer than 2^32 entries, and halving reaches at most
//!   [`LEAF_MAX`] entries within 31 more levels, so no leaf is deeper than 63.
//! - **No recursion.** Ranges wait on an explicit stack; ranges where all centres coincide are
//!   split at the median in key order.
//! - **Charged.** Each node charges its entry count to the meter before its work, so a build stops
//!   at the same node for the same budget on every machine.
//!
//! Method: I. Wald, "On fast construction of SAH-based bounding volume hierarchies", IEEE
//! Symposium on Interactive Ray Tracing (2007), <https://doi.org/10.1109/RT.2007.4342588>
//! (binning centroids into a fixed number of bins per axis and sweeping the bin boundaries), with
//! costs counted in whole grid points and ties broken by axis, plane and key (section 19.9 and
//! chapter 27's `geom-bvh` row); the subtree phase follows chapter 23.4's helper.
//!
//! Cost: O(n·D) per level of the tree, about log₂ n levels for spread boxes and at most
//! [`MAX_DEPTH`]; memory: the nodes, the order, one centre per entry, and per subtree task a copy
//! of its range's order. No recursion.

extern crate alloc;

use alloc::vec::Vec;

use geom_run::{ChunkPlan, Exec, Phase, WorkMeter};

use crate::bound::{Aabb, measure};
use crate::error::BvhError;
use crate::tree::{Entry, Node, NodeKind};

/// The bins per axis of the surface-area heuristic.
pub const BINS: usize = 16;

/// The most entries a leaf holds.
pub const LEAF_MAX: usize = 4;

/// The deepest a node can be: the root is at depth 0.
pub const MAX_DEPTH: usize = 64;

/// The depth from which every split is at the median in key order.
pub const MEDIAN_DEPTH: usize = 32;

/// The most entries a subtree built as one parallel task holds: ranges are split on the calling
/// thread until each holds at most this many.
pub const GRAIN: usize = 4096;

/// One subtree per chunk of the helper.
const PLAN: ChunkPlan = ChunkPlan::fixed::<1>();

/// The phase that builds the subtrees.
const SUBTREES: Phase = Phase::new("box tree: subtrees");

/// The binary point of a bin scale: a centre's bin is ⌊offset · scale / 2^48⌋.
const BIN_SHIFT: u32 = 48;

/// 2^48, a bin scale's unit. A range of c entries uses b = min(c, [`BINS`]) bins, with scale
/// ⌊b·2^48 / (extent + 1)⌋; an offset is at most the extent (at most 2^45), so
/// offset · scale < b·2^48 ≤ 2^52: the product never overflows a `u64`, and the bin is below b.
const BIN_UNIT: u64 = 1 << BIN_SHIFT;

/// An entry as the builder reads it: its box and twice its centre.
#[derive(Clone, Copy)]
struct Item<const D: usize> {
    lo: [i64; D],
    hi: [i64; D],
    centre: [i64; D],
}

/// A running box: empty until something is added.
#[derive(Clone, Copy)]
struct Bounds<const D: usize> {
    lo: [i64; D],
    hi: [i64; D],
}

impl<const D: usize> Bounds<D> {
    const EMPTY: Bounds<D> = Bounds {
        lo: [i64::MAX; D],
        hi: [i64::MIN; D],
    };

    fn add(&mut self, lo: &[i64; D], hi: &[i64; D]) {
        for (((l, h), x), y) in self.lo.iter_mut().zip(self.hi.iter_mut()).zip(lo).zip(hi) {
            if x < l {
                *l = *x;
            }
            if y > h {
                *h = *y;
            }
        }
    }

    fn join(&mut self, other: &Bounds<D>) {
        self.add(&other.lo, &other.hi);
    }

    fn is_empty(&self) -> bool {
        self.lo.iter().zip(&self.hi).any(|(l, h)| l > h)
    }

    fn aabb(&self) -> Aabb<D> {
        Aabb::from_corners(self.lo, self.hi)
    }
}

/// What a range of entries is known to cover: its box, and the box of its centres.
#[derive(Clone, Copy)]
struct Cover<const D: usize> {
    bbox: Bounds<D>,
    centres: Bounds<D>,
}

impl<const D: usize> Cover<D> {
    const EMPTY: Cover<D> = Cover {
        bbox: Bounds::EMPTY,
        centres: Bounds::EMPTY,
    };

    fn add(&mut self, item: &Item<D>) {
        self.bbox.add(&item.lo, &item.hi);
        self.centres.add(&item.centre, &item.centre);
    }
}

/// One bin of one axis: how many centres fell in it, and the box of their entries.
#[derive(Clone, Copy)]
struct Bin<const D: usize> {
    n: u32,
    bbox: Bounds<D>,
}

impl<const D: usize> Bin<D> {
    const EMPTY: Bin<D> = Bin {
        n: 0,
        bbox: Bounds::EMPTY,
    };

    fn join(&mut self, other: &Bin<D>) {
        self.n = self.n.saturating_add(other.n);
        self.bbox.join(&other.bbox);
    }
}

/// Which child of which node a range becomes.
#[derive(Clone, Copy)]
enum Slot {
    Root,
    Left(usize),
    Right(usize),
}

/// A range of the order waiting to be built, with what it covers.
#[derive(Clone, Copy)]
struct Frame<const D: usize> {
    lo: usize,
    hi: usize,
    depth: usize,
    slot: Slot,
    cover: Cover<D>,
}

/// A subtree left for the parallel phase: its range of the order, its depth, its placeholder
/// node and what it covers.
#[derive(Clone, Copy)]
pub(crate) struct Task<const D: usize> {
    lo: usize,
    hi: usize,
    depth: usize,
    node: usize,
    cover: Cover<D>,
}

fn index(i: usize) -> Result<u32, BvhError> {
    u32::try_from(i).map_err(|_| BvhError::TooManyEntries)
}

fn at<T: Copy>(items: &[T], i: usize) -> Result<T, BvhError> {
    items.get(i).copied().ok_or(BvhError::Defect)
}

fn at_u32<T: Copy>(items: &[T], i: u32) -> Result<T, BvhError> {
    at(items, usize::try_from(i).map_err(|_| BvhError::Defect)?)
}

/// The nodes and the leaf order of the tree over `sorted` (entries in key order, keys distinct).
pub(crate) fn build<const D: usize>(
    sorted: &[Entry<D>],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<Node<D>>, Vec<u32>), BvhError> {
    let n = sorted.len();
    let mut nodes = Vec::new();
    let mut order = Vec::new();
    if n == 0 {
        return Ok((nodes, order));
    }
    order
        .try_reserve_exact(n)
        .map_err(|_| BvhError::OutOfMemory)?;
    let mut items = Vec::new();
    items
        .try_reserve_exact(n)
        .map_err(|_| BvhError::OutOfMemory)?;
    let mut cover = Cover::EMPTY;
    for (i, e) in sorted.iter().enumerate() {
        order.push(index(i)?);
        let item = Item {
            lo: e.bbox.lo(),
            hi: e.bbox.hi(),
            centre: core::array::from_fn(|axis| e.bbox.centre2(axis)),
        };
        cover.add(&item);
        items.push(item);
    }
    let mut tasks = Vec::new();
    let mut scratch = Vec::new();
    let root = Task {
        lo: 0,
        hi: n,
        depth: 0,
        node: 0,
        cover,
    };
    grow(
        &items,
        &mut order,
        root,
        Some(GRAIN),
        &mut nodes,
        &mut tasks,
        &mut scratch,
        meter,
    )?;
    let subtrees = {
        let shared: &[u32] = &order;
        exec.try_map(meter, SUBTREES, PLAN, &tasks, |_, task: &Task<D>, meter| {
            let mut ids = Vec::new();
            let range = shared.get(task.lo..task.hi).ok_or(BvhError::Defect)?;
            ids.try_reserve_exact(range.len())
                .map_err(|_| BvhError::OutOfMemory)?;
            ids.extend_from_slice(range);
            let mut sub = Vec::new();
            let (mut none, mut scratch) = (Vec::new(), Vec::new());
            grow(
                &items,
                &mut ids,
                *task,
                None,
                &mut sub,
                &mut none,
                &mut scratch,
                meter,
            )?;
            Ok((sub, ids))
        })?
    };
    for (task, (sub, ids)) in tasks.iter().zip(subtrees) {
        order
            .get_mut(task.lo..task.hi)
            .ok_or(BvhError::Defect)?
            .copy_from_slice(&ids);
        splice(&mut nodes, task.node, &sub)?;
    }
    Ok((nodes, order))
}

/// Puts subtree `sub` in place of placeholder `node`: its root replaces the placeholder and the
/// rest follow the nodes already there, in their own order, children renumbered.
fn splice<const D: usize>(
    nodes: &mut Vec<Node<D>>,
    node: usize,
    sub: &[Node<D>],
) -> Result<(), BvhError> {
    let base = nodes.len();
    let map = |j: u32| -> Result<u32, BvhError> {
        if j == 0 {
            return index(node);
        }
        let j = usize::try_from(j).map_err(|_| BvhError::Defect)?;
        index(
            base.checked_add(j)
                .and_then(|k| k.checked_sub(1))
                .ok_or(BvhError::Defect)?,
        )
    };
    nodes
        .try_reserve(sub.len().saturating_sub(1))
        .map_err(|_| BvhError::OutOfMemory)?;
    for (j, s) in sub.iter().enumerate() {
        let kind = match s.kind() {
            NodeKind::Inner { left, right } => NodeKind::Inner {
                left: map(left)?,
                right: map(right)?,
            },
            leaf @ NodeKind::Leaf { .. } => leaf,
        };
        let renumbered = Node::new(s.bbox(), kind);
        if j == 0 {
            *nodes.get_mut(node).ok_or(BvhError::Defect)? = renumbered;
        } else {
            nodes.push(renumbered);
        }
    }
    Ok(())
}

/// Builds `top`'s range of the order (`ids` holds positions `top.lo..top.hi`) into `nodes`. With a
/// grain, a range of at most that many entries (and more than a leaf's) becomes a placeholder node
/// and a task instead.
#[allow(
    clippy::too_many_arguments,
    reason = "the builder's state, passed rather than bundled so the subtree tasks share the read-only parts"
)]
fn grow<const D: usize>(
    items: &[Item<D>],
    ids: &mut [u32],
    top: Task<D>,
    grain: Option<usize>,
    nodes: &mut Vec<Node<D>>,
    tasks: &mut Vec<Task<D>>,
    scratch: &mut Vec<u32>,
    meter: &mut WorkMeter<'_>,
) -> Result<(), BvhError> {
    let base = top.lo;
    let mut stack = Vec::new();
    stack
        .try_reserve(MAX_DEPTH.saturating_add(2))
        .map_err(|_| BvhError::OutOfMemory)?;
    stack.push(Frame {
        lo: 0,
        hi: ids.len(),
        depth: top.depth,
        slot: Slot::Root,
        cover: top.cover,
    });
    while let Some(f) = stack.pop() {
        if f.depth > MAX_DEPTH || f.cover.bbox.is_empty() {
            return Err(BvhError::Defect);
        }
        let count = f.hi.checked_sub(f.lo).ok_or(BvhError::Defect)?;
        meter.charge(u64::try_from(count).map_err(|_| BvhError::Defect)?)?;
        let idx = nodes.len();
        let start = base.checked_add(f.lo).ok_or(BvhError::Defect)?;
        nodes.try_reserve(1).map_err(|_| BvhError::OutOfMemory)?;
        let bbox = f.cover.bbox.aabb();
        nodes.push(Node::new(
            bbox,
            NodeKind::Leaf {
                start: index(start)?,
                len: index(count)?,
            },
        ));
        if let Slot::Left(parent) | Slot::Right(parent) = f.slot {
            let p = nodes.get_mut(parent).ok_or(BvhError::Defect)?;
            let kind = match (p.kind(), f.slot) {
                (NodeKind::Inner { right, .. }, Slot::Left(_)) => NodeKind::Inner {
                    left: index(idx)?,
                    right,
                },
                (NodeKind::Inner { left, .. }, _) => NodeKind::Inner {
                    left,
                    right: index(idx)?,
                },
                (NodeKind::Leaf { .. }, _) => return Err(BvhError::Defect),
            };
            *p = Node::new(p.bbox(), kind);
        }
        if count <= LEAF_MAX {
            continue;
        }
        if grain.is_some_and(|g| count <= g) {
            tasks.try_reserve(1).map_err(|_| BvhError::OutOfMemory)?;
            tasks.push(Task {
                lo: start,
                hi: base.checked_add(f.hi).ok_or(BvhError::Defect)?,
                depth: f.depth,
                node: idx,
                cover: f.cover,
            });
            continue;
        }
        let range = ids.get_mut(f.lo..f.hi).ok_or(BvhError::Defect)?;
        let (mid, left, right) = split(items, range, f.depth, &f.cover.centres, scratch)?;
        *nodes.get_mut(idx).ok_or(BvhError::Defect)? =
            Node::new(bbox, NodeKind::Inner { left: 0, right: 0 });
        let cut = f.lo.checked_add(mid).ok_or(BvhError::Defect)?;
        let depth = f.depth.checked_add(1).ok_or(BvhError::Defect)?;
        stack.push(Frame {
            lo: cut,
            hi: f.hi,
            depth,
            slot: Slot::Right(idx),
            cover: right,
        });
        stack.push(Frame {
            lo: f.lo,
            hi: cut,
            depth,
            slot: Slot::Left(idx),
            cover: left,
        });
    }
    Ok(())
}

/// The bin of a centre `offset` above the range's lowest, for a bin scale of `scale` and `used`
/// bins: ⌊offset · scale / 2^48⌋, at most `used − 1` (the product: see `BIN_UNIT`).
fn bin(offset: i64, scale: u64, used: usize) -> usize {
    let o = u64::try_from(offset).unwrap_or(0);
    let b = o.wrapping_mul(scale).wrapping_shr(BIN_SHIFT);
    usize::try_from(b)
        .unwrap_or(used)
        .min(used.saturating_sub(1))
}

/// The covers of `ids[..mid]` and `ids[mid..]`.
fn covers<const D: usize>(
    items: &[Item<D>],
    ids: &[u32],
    mid: usize,
) -> Result<(Cover<D>, Cover<D>), BvhError> {
    let (mut left, mut right) = (Cover::EMPTY, Cover::EMPTY);
    for (i, &id) in ids.iter().enumerate() {
        let item = at_u32(items, id)?;
        if i < mid {
            left.add(&item);
        } else {
            right.add(&item);
        }
    }
    Ok((left, right))
}

/// Splits `ids` (more than a leaf's entries, in key order, their centres within `centres`) stably
/// into a left and a right part, and returns the left part's length (at least 1 and below
/// `ids.len()`) and what each part covers.
fn split<const D: usize>(
    items: &[Item<D>],
    ids: &mut [u32],
    depth: usize,
    centres: &Bounds<D>,
    scratch: &mut Vec<u32>,
) -> Result<(usize, Cover<D>, Cover<D>), BvhError> {
    let count = ids.len();
    let half = count / 2;
    if depth >= MEDIAN_DEPTH {
        let (l, r) = covers(items, ids, half)?;
        return Ok((half, l, r));
    }
    // A small range gets fewer bins: as many as it has entries, up to BINS.
    let used = BINS.min(count);
    let unit = BIN_UNIT.saturating_mul(u64::try_from(used).map_err(|_| BvhError::Defect)?);
    // Each axis whose centres differ, with its bin scale; 0 marks an axis with no spread.
    let mut scales = [0u64; D];
    for ((s, lo), hi) in scales.iter_mut().zip(&centres.lo).zip(&centres.hi) {
        if hi > lo {
            // hi − lo ≤ 2^45: centres are sums of two coordinates within ±2^43.
            let extent = u64::try_from(hi.saturating_sub(*lo)).map_err(|_| BvhError::Defect)?;
            *s = unit
                .checked_div(extent.saturating_add(1))
                .ok_or(BvhError::Defect)?;
        }
    }
    let mut bins = [[Bin::<D>::EMPTY; BINS]; D];
    for &id in ids.iter() {
        let item = at_u32(items, id)?;
        for (((axis_bins, scale), c), lo) in bins
            .iter_mut()
            .zip(&scales)
            .zip(&item.centre)
            .zip(&centres.lo)
        {
            if *scale == 0 {
                continue;
            }
            let slot = axis_bins
                .get_mut(bin(c.saturating_sub(*lo), *scale, used))
                .ok_or(BvhError::Defect)?;
            slot.n = slot.n.saturating_add(1);
            slot.bbox.add(&item.lo, &item.hi);
        }
    }
    // (cost, axis, plane) of the best plane so far; the lower axis, then plane, on a tie.
    let mut best: Option<(u128, usize, usize)> = None;
    for (axis, (axis_bins, scale)) in bins.iter().zip(&scales).enumerate() {
        if *scale == 0 {
            continue;
        }
        // right[i]: the bins i..used together, and that side's cost, measure · count (below 2^124:
        // a measure below 2^92 times fewer than 2^32 entries).
        let mut right = [(0u32, 0u128); BINS];
        let mut acc = Bin::EMPTY;
        for (r, b) in right.iter_mut().zip(axis_bins).take(used).rev() {
            acc.join(b);
            *r = (
                acc.n,
                measure(&acc.bbox.lo, &acc.bbox.hi).wrapping_mul(u128::from(acc.n)),
            );
        }
        let mut left = Bin::EMPTY;
        for (plane, (b, &(rn, rcost))) in axis_bins
            .iter()
            .zip(right.iter().skip(1))
            .take(used.saturating_sub(1))
            .enumerate()
        {
            left.join(b);
            if left.n == 0 || rn == 0 {
                continue;
            }
            let cost = measure(&left.bbox.lo, &left.bbox.hi)
                .wrapping_mul(u128::from(left.n))
                .wrapping_add(rcost);
            if best.is_none_or(|(c, ..)| cost < c) {
                best = Some((cost, axis, plane));
            }
        }
    }
    let Some((_, axis, plane)) = best else {
        let (l, r) = covers(items, ids, half)?;
        return Ok((half, l, r));
    };
    let scale = at(&scales, axis)?;
    let lo = at(&centres.lo, axis)?;
    scratch.clear();
    scratch
        .try_reserve(count)
        .map_err(|_| BvhError::OutOfMemory)?;
    let (mut left, mut right) = (Cover::EMPTY, Cover::EMPTY);
    let mut kept = 0usize;
    for i in 0..count {
        let id = at(ids, i)?;
        let item = at_u32(items, id)?;
        if bin(at(&item.centre, axis)?.saturating_sub(lo), scale, used) <= plane {
            *ids.get_mut(kept).ok_or(BvhError::Defect)? = id;
            kept = kept.saturating_add(1);
            left.add(&item);
        } else {
            scratch.push(id);
            right.add(&item);
        }
    }
    ids.get_mut(kept..)
        .ok_or(BvhError::Defect)?
        .copy_from_slice(scratch);
    if kept == 0 || kept == count {
        return Err(BvhError::Defect);
    }
    Ok((kept, left, right))
}
