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

/// An entry as the builder moves it: its box and its position in key order.
#[derive(Clone, Copy)]
struct Item<const D: usize> {
    lo: [i64; D],
    hi: [i64; D],
    id: u32,
}

impl<const D: usize> Item<D> {
    /// Twice the centre on `axis`, lo + hi (|lo + hi| ≤ 2^44, so it never wraps).
    #[inline]
    fn centre(&self, axis: usize) -> i64 {
        match (self.lo.get(axis), self.hi.get(axis)) {
            (Some(l), Some(h)) => l.wrapping_add(*h),
            _ => 0,
        }
    }
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

    #[inline]
    fn add(&mut self, lo: &[i64; D], hi: &[i64; D]) {
        for (l, x) in self.lo.iter_mut().zip(lo) {
            *l = (*l).min(*x);
        }
        for (h, y) in self.hi.iter_mut().zip(hi) {
            *h = (*h).max(*y);
        }
    }

    #[inline]
    fn join(&mut self, other: &Bounds<D>) {
        self.add(&other.lo, &other.hi);
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

    #[inline]
    fn add(&mut self, item: &Item<D>) {
        self.bbox.add(&item.lo, &item.hi);
        let c: [i64; D] = core::array::from_fn(|axis| item.centre(axis));
        self.centres.add(&c, &c);
    }

    fn of(items: &[Item<D>]) -> Cover<D> {
        let mut c = Cover::EMPTY;
        for item in items {
            c.add(item);
        }
        c
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

/// A range of the items waiting to be built, with what it covers.
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

/// The nodes and the leaf order of the tree over `sorted` (entries in key order, keys distinct):
/// `order[p]` is the position in `sorted` of the entry at leaf position p.
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
    let mut items = Vec::new();
    items
        .try_reserve_exact(n)
        .map_err(|_| BvhError::OutOfMemory)?;
    for (i, e) in sorted.iter().enumerate() {
        items.push(Item {
            lo: e.bbox.lo(),
            hi: e.bbox.hi(),
            id: index(i)?,
        });
    }
    let mut tasks = Vec::new();
    let mut scratch = Vec::new();
    let root = Task {
        lo: 0,
        hi: n,
        depth: 0,
        node: 0,
        cover: Cover::of(&items),
    };
    grow(
        &mut items,
        root,
        Some(GRAIN),
        &mut nodes,
        &mut tasks,
        &mut scratch,
        meter,
    )?;
    let subtrees = {
        let shared: &[Item<D>] = &items;
        exec.try_map(meter, SUBTREES, PLAN, &tasks, |_, task: &Task<D>, meter| {
            let range = shared.get(task.lo..task.hi).ok_or(BvhError::Defect)?;
            let mut local = Vec::new();
            local
                .try_reserve_exact(range.len())
                .map_err(|_| BvhError::OutOfMemory)?;
            local.extend_from_slice(range);
            let mut sub = Vec::new();
            let (mut none, mut scratch) = (Vec::new(), Vec::new());
            grow(
                &mut local,
                *task,
                None,
                &mut sub,
                &mut none,
                &mut scratch,
                meter,
            )?;
            Ok((sub, local))
        })?
    };
    for (task, (sub, local)) in tasks.iter().zip(subtrees) {
        items
            .get_mut(task.lo..task.hi)
            .ok_or(BvhError::Defect)?
            .copy_from_slice(&local);
        splice(&mut nodes, task.node, &sub)?;
    }
    order
        .try_reserve_exact(n)
        .map_err(|_| BvhError::OutOfMemory)?;
    order.extend(items.iter().map(|item| item.id));
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

/// Builds `top`'s range (`items` holds the leaf positions `top.lo..top.hi`) into `nodes`. With a
/// grain, a range of at most that many entries (and more than a leaf's) becomes a placeholder node
/// and a task instead.
fn grow<const D: usize>(
    items: &mut [Item<D>],
    top: Task<D>,
    grain: Option<usize>,
    nodes: &mut Vec<Node<D>>,
    tasks: &mut Vec<Task<D>>,
    scratch: &mut Vec<Item<D>>,
    meter: &mut WorkMeter<'_>,
) -> Result<(), BvhError> {
    let base = top.lo;
    let mut stack = Vec::new();
    stack
        .try_reserve(MAX_DEPTH.saturating_add(2))
        .map_err(|_| BvhError::OutOfMemory)?;
    stack.push(Frame {
        lo: 0,
        hi: items.len(),
        depth: top.depth,
        slot: Slot::Root,
        cover: top.cover,
    });
    while let Some(f) = stack.pop() {
        if f.depth > MAX_DEPTH {
            return Err(BvhError::Defect);
        }
        let count = f.hi.checked_sub(f.lo).ok_or(BvhError::Defect)?;
        meter.charge(u64::try_from(count).map_err(|_| BvhError::Defect)?)?;
        let idx = nodes.len();
        let start = base.checked_add(f.lo).ok_or(BvhError::Defect)?;
        let bbox = f.cover.bbox.aabb();
        nodes.try_reserve(1).map_err(|_| BvhError::OutOfMemory)?;
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
        let range = items.get_mut(f.lo..f.hi).ok_or(BvhError::Defect)?;
        let (mid, left, right) = split(range, f.depth, &f.cover.centres, scratch)?;
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
#[inline]
fn bin(offset: i64, scale: u64, used: usize) -> usize {
    let o = u64::try_from(offset).unwrap_or(0);
    let b = o.wrapping_mul(scale).wrapping_shr(BIN_SHIFT);
    usize::try_from(b)
        .unwrap_or(used)
        .min(used.saturating_sub(1))
}

/// Splits `items` (more than a leaf's entries, in key order, their centres within `centres`)
/// stably into a left and a right part, and returns the left part's length (at least 1 and below
/// `items.len()`) and what each part covers.
fn split<const D: usize>(
    items: &mut [Item<D>],
    depth: usize,
    centres: &Bounds<D>,
    scratch: &mut Vec<Item<D>>,
) -> Result<(usize, Cover<D>, Cover<D>), BvhError> {
    let count = items.len();
    let half = count / 2;
    let median = |items: &[Item<D>]| -> Result<(usize, Cover<D>, Cover<D>), BvhError> {
        let (l, r) = items.split_at_checked(half).ok_or(BvhError::Defect)?;
        Ok((half, Cover::of(l), Cover::of(r)))
    };
    if depth >= MEDIAN_DEPTH {
        return median(items);
    }
    // The axis along which the centres spread most, the lowest on a tie (Wald 2007 bins along
    // that axis alone); a range whose centres all coincide is split at the median.
    let mut axis = 0usize;
    let mut spread = 0i64;
    for (a, (lo, hi)) in centres.lo.iter().zip(&centres.hi).enumerate() {
        let s = hi.wrapping_sub(*lo);
        if s > spread {
            spread = s;
            axis = a;
        }
    }
    if spread <= 0 {
        return median(items);
    }
    // A small range gets fewer bins: as many as it has entries, up to BINS. The spread is at most
    // 2^45 (centres are sums of two coordinates within ±2^43).
    let used = BINS.min(count);
    let unit = BIN_UNIT.saturating_mul(u64::try_from(used).map_err(|_| BvhError::Defect)?);
    let scale = unit
        .checked_div(
            u64::try_from(spread)
                .map_err(|_| BvhError::Defect)?
                .saturating_add(1),
        )
        .ok_or(BvhError::Defect)?;
    let lo = centres.lo.get(axis).copied().ok_or(BvhError::Defect)?;
    let mut bins = [Bin::<D>::EMPTY; BINS];
    for item in items.iter() {
        if let Some(slot) = bins.get_mut(bin(item.centre(axis).wrapping_sub(lo), scale, used)) {
            slot.n = slot.n.wrapping_add(1);
            slot.bbox.add(&item.lo, &item.hi);
        }
    }
    // right[i]: the bins i..used together, and that side's cost, measure · count (below 2^124: a
    // measure below 2^92 times fewer than 2^32 entries).
    let mut right = [(0u32, 0u128); BINS];
    let mut acc = Bin::EMPTY;
    for (r, b) in right.iter_mut().zip(&bins).take(used).rev() {
        acc.join(b);
        *r = (
            acc.n,
            measure(&acc.bbox.lo, &acc.bbox.hi).wrapping_mul(u128::from(acc.n)),
        );
    }
    // The plane of least cost, the lower plane on a tie.
    let mut best: Option<(u128, usize)> = None;
    let mut left = Bin::EMPTY;
    for (plane, (b, &(rn, rcost))) in bins
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
        if best.is_none_or(|(c, _)| cost < c) {
            best = Some((cost, plane));
        }
    }
    let Some((_, plane)) = best else {
        return median(items);
    };
    // A stable partition: the left part's items stay in order at the front, the right part's go
    // to the scratch buffer in order and then follow them. Reads go by iterator, so no item costs
    // a fallible index.
    scratch.clear();
    scratch
        .try_reserve(count)
        .map_err(|_| BvhError::OutOfMemory)?;
    let (mut left, mut right) = (Cover::EMPTY, Cover::EMPTY);
    let mut kept = 0usize;
    for i in 0..count {
        let Some(&item) = items.get(i) else { break };
        if bin(item.centre(axis).wrapping_sub(lo), scale, used) <= plane {
            if let Some(slot) = items.get_mut(kept) {
                *slot = item;
            }
            kept = kept.wrapping_add(1);
            left.add(&item);
        } else {
            scratch.push(item);
            right.add(&item);
        }
    }
    items
        .get_mut(kept..)
        .ok_or(BvhError::Defect)?
        .copy_from_slice(scratch);
    if kept == 0 || kept == count {
        return Err(BvhError::Defect);
    }
    Ok((kept, left, right))
}
