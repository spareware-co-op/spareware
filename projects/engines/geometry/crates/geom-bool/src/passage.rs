//! The narrowest passage between two seeds of one cell (11.7).
//!
//! What it does: two seeds in one cell mean the rooms they name leak into each other. [`narrowest`]
//! finds the path between them whose narrowest point is widest, and reports that point and its
//! width, so a gap of 0.003 mm where two walls fail to meet shows up at once, in its place.
//!
//! How it runs: the grid lines of the cell's own corners cut its box into blocks, which meet across
//! portals. A block belongs to the cell if its centre does, by an exact trace. From the first seed's
//! block a best-first walk takes the widest open portal next, which is Dijkstra's algorithm with the
//! smallest width on the way in place of a sum, and stops at the second seed's block. A portal's
//! width is the shorter of its two sides.
//!
//! Guarantees:
//!
//! - **A measurement for people, not a decision.** Nothing here changes a result. On walls square to
//!   the axes the blocks fit the cell and the width is exact; on slanted walls a block is judged by
//!   its centre, so the width is approximate, and it is labelled so (11.7).
//! - **Exact where it decides, deterministic everywhere.** Each block is placed by exact winding
//!   numbers; ties fall to the lowest block.
//! - **The gap is in the grid.** Every corner's coordinates are grid lines, so a gap of any width
//!   between two walls is a row of blocks of that width.
//!
//! Method: chapter 11.7 (`projects/engines/geometry/architecture/11-booleans.md`); the widest path
//! is the maximum bottleneck path of Pollack, "The maximum capacity route through a network",
//! Operations Research 8, 1960, <https://doi.org/10.1287/opre.8.5.733>, here over Dijkstra's walk.
//!
//! Cost: O(b log b) over the b blocks walked, each traced through every polygon. Charges the meter
//! one unit per polygon per path segment.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use geom_num::plane::Plane;
use geom_num::{NumError, P3};
use geom_poly::convex::Piece;
use geom_poly::subdivide::around;
use geom_poly::trace::{Polygon, TraceError, winding_at};
use geom_poly::winding::Winding;
use geom_run::WorkMeter;

/// Where a leak is narrowest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Passage {
    /// Its width, in grid steps, labelled Approximate (11.7).
    pub width: i64,
    /// A grid point in it.
    pub at: [i64; 3],
}

/// The grid lines of one axis: the corners' coordinates and the box's ends.
fn lines(points: &[[i64; 3]], axis: usize) -> Vec<i64> {
    let mut out: Vec<i64> = points.iter().filter_map(|p| p.get(axis).copied()).collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The block holding `v` along one axis: the last line at or below it.
fn block_of(lines: &[i64], v: i64) -> usize {
    lines.partition_point(|line| *line <= v).saturating_sub(1)
}

/// The plane through the middle of block `at` on `axis`.
fn middle(lines: &[i64], at: usize, axis: usize) -> Result<Plane, NumError> {
    let low = lines.get(at).copied().ok_or(NumError::Degenerate)?;
    let high = lines
        .get(at.saturating_add(1))
        .copied()
        .ok_or(NumError::Degenerate)?;
    let normal = [0usize, 1, 2].map(|k| if k == axis { 2 } else { 0 });
    Plane::new(
        normal,
        i128::from(low)
            .saturating_add(i128::from(high))
            .saturating_neg(),
    )
}

/// The width of block `at` on `axis`.
fn width(lines: &[i64], at: usize) -> i64 {
    let low = lines.get(at).copied().unwrap_or(0);
    lines
        .get(at.saturating_add(1))
        .map_or(0, |high| high.saturating_sub(low))
}

/// The grid point of `p`.
fn coordinates(p: P3) -> [i64; 3] {
    [p.x.get(), p.y.get(), p.z.get()]
}

/// The low corner of `block`.
fn corner_of(block: [usize; 3], grid: &[Vec<i64>; 3]) -> [i64; 3] {
    [0usize, 1, 2].map(|axis| {
        let list = grid.get(axis).map_or(&[][..], Vec::as_slice);
        list.get(block.get(axis).copied().unwrap_or(0))
            .copied()
            .unwrap_or(0)
    })
}

/// The blocks next to `block`, each with the width of the portal to it: the shorter of the two
/// sides they share.
fn neighbours(block: [usize; 3], grid: &[Vec<i64>; 3]) -> Vec<([usize; 3], i64)> {
    let size = |of: [usize; 3]| {
        [0usize, 1, 2].map(|axis| {
            let list = grid.get(axis).map_or(&[][..], Vec::as_slice);
            width(list, of.get(axis).copied().unwrap_or(0))
        })
    };
    let mut out = Vec::with_capacity(6);
    for axis in 0..3 {
        let at = block.get(axis).copied().unwrap_or(0);
        let count = grid.get(axis).map_or(0, Vec::len).saturating_sub(1);
        for next_at in [at.checked_sub(1), Some(at.saturating_add(1))] {
            let Some(next_at) = next_at.filter(|n| *n < count) else {
                continue;
            };
            let mut next = block;
            next.iter_mut()
                .skip(axis)
                .take(1)
                .for_each(|slot| *slot = next_at);
            let (here, there) = (size(block), size(next));
            let across = (1..3)
                .map(|shift| axis.saturating_add(shift) % 3)
                .filter_map(|side| here.get(side).zip(there.get(side)))
                .map(|(a, b)| (*a).min(*b))
                .min()
                .unwrap_or(0);
            out.push((next, across));
        }
    }
    out
}

/// The narrowest point of the widest path from `from` to `to` through the cell `pieces` bound.
/// `None` if the blocks do not join them: the two lie in the cell but a slanted wall hides the way
/// from every block's centre.
///
/// # Errors
///
/// A [`TraceError`]: a stopped meter, or what the numbers layer refuses.
pub fn narrowest(
    pieces: &[Piece],
    corners: &[P3],
    from: P3,
    to: P3,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Passage>, TraceError> {
    let polygons: Vec<Polygon> = pieces
        .iter()
        .map(|piece| Polygon {
            piece: piece.clone(),
            entry: Some(0),
        })
        .collect();
    let cell = around(&polygons)?;
    let mut points: Vec<[i64; 3]> = corners.iter().copied().map(coordinates).collect();
    points.push(coordinates(from));
    points.push(coordinates(to));
    let grid = [lines(&points, 0), lines(&points, 1), lines(&points, 2)];
    let [xs, ys, zs] = &grid;
    let start = [
        block_of(xs, from.x.get()),
        block_of(ys, from.y.get()),
        block_of(zs, from.z.get()),
    ];
    let goal = [
        block_of(xs, to.x.get()),
        block_of(ys, to.y.get()),
        block_of(zs, to.z.get()),
    ];
    let mut inside: BTreeMap<[usize; 3], bool> = BTreeMap::new();
    let mut open = |block: [usize; 3], meter: &mut WorkMeter<'_>| -> Result<bool, TraceError> {
        if let Some(held) = inside.get(&block) {
            return Ok(*held);
        }
        let [i, j, k] = block;
        let planes = [middle(xs, i, 0)?, middle(ys, j, 1)?, middle(zs, k, 2)?];
        let here = winding_at(&cell, &Winding::new(), &polygons, planes, meter)?;
        let held = here.count(0) > 0;
        inside.insert(block, held);
        Ok(held)
    };
    let mut best: BTreeMap<[usize; 3], i64> = BTreeMap::new();
    let mut pinch: BTreeMap<[usize; 3], Option<Passage>> = BTreeMap::new();
    let mut queue = BTreeSet::new();
    queue.insert((i64::MAX, start));
    best.insert(start, i64::MAX);
    pinch.insert(start, None);
    while let Some((reach, block)) = queue.pop_last() {
        if block == goal {
            return Ok(pinch.get(&block).copied().flatten());
        }
        if best.get(&block).is_some_and(|held| *held > reach) {
            continue;
        }
        for (next, across) in neighbours(block, &grid) {
            if !open(next, meter)? {
                continue;
            }
            let wide = reach.min(across);
            if best.get(&next).is_none_or(|held| *held < wide) {
                best.insert(next, wide);
                let before = pinch.get(&block).copied().flatten();
                let here = if before.is_none_or(|held| across < held.width) {
                    Some(Passage {
                        width: across,
                        at: corner_of(next, &grid),
                    })
                } else {
                    before
                };
                pinch.insert(next, here);
                queue.insert((wide, next));
            }
        }
    }
    Ok(None)
}
