//! Adaptive subdivision and the parallel task tree: steps 1 and 4 of 11.3, around the local work.
//!
//! What it does: [`arrange`] takes the polygons of an arrangement and classifies every piece. A
//! box is split in two along its longest axis until it holds few polygons; polygons across the
//! split are clipped exactly, and the upper box's reference point is walked up from the lower's,
//! adding up the polygons it crosses, so each leaf can be solved on its own: [`local_cuts`], then
//! [`classify`]. A box whose every test is settled by its reference vector alone, whatever the
//! polygons in it do, is dropped (step 4).
//!
//! Boxes split on odd half steps. Every polygon that comes from grid data lies on a whole step or
//! is tilted, so no polygon lies in a split plane, and every polygon ending on a piece reaches it
//! inside one leaf. A plane that does hold a polygon, such as a committed plane, is passed over for
//! the next odd half step.
//!
//! Guarantees:
//!
//! - **The same pieces whatever the threads.** Each level of the tree is one phase of the parallel
//!   helper, and the leaves are solved as another, in tree order, so the output is the same bits
//!   on any number of threads, and a failure the one a serial walk meets first (23.4).
//! - **No recursion.** The tree is walked level by level, so its depth never threatens a
//!   WebAssembly stack (principle 13). A box holding polygons that a split no longer separates,
//!   such as many faces on one plane, stops as a leaf instead of dividing down to the grid.
//! - **Early outs are mechanical.** A box is dropped only when every test takes the same answer
//!   for every count of every entry with a polygon in it (11.3, step 4). A box with a cutter in it
//!   is kept, and with no tests nothing is dropped.
//!
//! Method: chapter 11.3's five steps (`projects/engines/geometry/architecture/11-booleans.md`),
//! after EMBER's sections 4.1, 4.2 and 4.5 (Trettner, Nehring-Wirxel and Kobbelt 2022,
//! <https://doi.org/10.1145/3528223.3530181>); Kleene's three-valued logic for the early outs,
//! in [`crate::winding`]; the phases of chapter 23.4.
//!
//! Cost: O(n) clips per level for n polygons across the level's boxes, and at most 123 levels:
//! a box of the frame halves 41 times along each axis. Each leaf costs what [`local_cuts`] and
//! [`classify`] cost on its polygons. Charges the meter one unit per polygon a split clips.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Reverse;

use geom_num::NumError;
use geom_num::plane::{Plane, Tpi};
use geom_run::{ChunkPlan, Exec, Phase, RunError, WorkMeter};

use crate::convex::Piece;
use crate::cut::{Clipped, clip, local_cuts};
use crate::trace::{Cell, Classified, Polygon, TraceError, classify, reference_above};
use crate::winding::{Entry, Winding, WindingTest};

/// How many polygons a leaf holds at most, unless a split no longer separates them: the size at
/// which local cuts, quadratic in the polygons, cost about what another level of boxes would.
pub const LEAF_POLYGONS: usize = 32;

/// One box at a time: a box's cost varies too much for a chunk of them to mean anything.
const ONE_BOX: ChunkPlan = ChunkPlan::fixed::<1>();

/// One level of the tree.
const SPLITS: Phase = Phase::new("arrangement: split boxes");

/// The leaves.
const LEAVES: Phase = Phase::new("arrangement: solve leaves");

/// A box of the tree.
struct Node {
    /// Its place in tree order: the bits of its path from the top bit down, 1 for the upper box.
    key: u128,
    /// How many splits are above it.
    depth: u32,
    cell: Cell,
    /// The vector at its reference point.
    reference: Winding,
    polygons: Vec<Polygon>,
    /// The index in the input of each polygon.
    sources: Vec<usize>,
}

/// What a level does to a box.
enum Outcome {
    /// Every test is settled in it.
    Dropped,
    /// It stays whole.
    Leaf,
    /// The lower box and the upper box.
    Split(Box<(Node, Node)>),
}

/// The box around `polygons`, in half steps, a step clear of them with its faces on odd half
/// steps, so that no polygon lies in one.
///
/// # Errors
///
/// [`NumError::Degenerate`] for a piece whose adjacent bounds do not meet, [`NumError::OutOfRange`]
/// for a corner outside the frame.
pub fn around(polygons: &[Polygon]) -> Result<Cell, NumError> {
    let mut low = [i64::MAX; 3];
    let mut high = [i64::MIN; 3];
    for polygon in polygons {
        let Piece { face, bounds } = &polygon.piece;
        let after = bounds.iter().cycle().skip(1);
        for (bound, next) in bounds.iter().zip(after) {
            let at = Tpi::meeting(*face, *bound, *next)?.to_grid()?.value();
            let coordinates = [at.x.get(), at.y.get(), at.z.get()];
            for ((l, h), v) in low.iter_mut().zip(&mut high).zip(coordinates) {
                *l = (*l).min(v);
                *h = (*h).max(v);
            }
        }
    }
    let half = |v: i64, by: i64| v.saturating_mul(2).saturating_add(by);
    if polygons.is_empty() {
        return Ok(Cell {
            low: [-1; 3],
            high: [1; 3],
        });
    }
    Ok(Cell {
        low: low.map(|v| half(v, -3)),
        high: high.map(|v| half(v, 3)),
    })
}

/// The odd half steps to try splitting `cell` at, as (axis, half step), the longest axis first.
fn splits(cell: &Cell) -> [(usize, Option<i64>); 3] {
    let at = |axis: usize| {
        let (low, high) = (
            cell.low.get(axis).copied().unwrap_or(0),
            cell.high.get(axis).copied().unwrap_or(0),
        );
        let middle = low.saturating_add(high.saturating_sub(low).saturating_div(2));
        let odd = middle | 1;
        (low < odd && odd < high).then_some(odd)
    };
    let width = |axis: usize| {
        let (low, high) = (cell.low.get(axis), cell.high.get(axis));
        low.zip(high).map_or(0, |(l, h)| h.saturating_sub(*l))
    };
    let mut axes = [0, 1, 2];
    axes.sort_by_key(|axis| Reverse(width(*axis)));
    axes.map(|axis| (axis, at(axis)))
}

/// Whether every test is settled in `node`, whatever its polygons do.
fn settled(node: &Node, tests: &[WindingTest]) -> bool {
    if tests.is_empty() || node.polygons.iter().any(|p| p.entry.is_none()) {
        return false;
    }
    let mut open: Vec<Entry> = node.polygons.iter().filter_map(|p| p.entry).collect();
    open.sort_unstable();
    open.dedup();
    tests
        .iter()
        .all(|test| test.decided(&node.reference, &open).is_some())
}

/// What this level does to `node`.
fn divide(
    node: &Node,
    leaf: usize,
    tests: &[WindingTest],
    meter: &mut WorkMeter<'_>,
) -> Result<Outcome, TraceError> {
    if settled(node, tests) {
        return Ok(Outcome::Dropped);
    }
    let count = node.polygons.len();
    if count <= leaf {
        return Ok(Outcome::Leaf);
    }
    // Faces that span every split are not separated by more of them.
    let enough = count
        .saturating_mul(2)
        .saturating_sub(count.saturating_div(8));
    for (axis, at) in splits(&node.cell) {
        let Some(at) = at else {
            continue;
        };
        let n = [0, 1, 2].map(|i| if i == axis { 2 } else { 0 });
        let plane = Plane::new(n, i128::from(at).saturating_neg())?;
        let mut below = (Vec::new(), Vec::new());
        let mut above = (Vec::new(), Vec::new());
        let mut in_plane = false;
        for (polygon, source) in node.polygons.iter().zip(&node.sources) {
            meter.charge(1)?;
            let part = |piece: Piece| Polygon {
                piece,
                entry: polygon.entry,
            };
            match clip(&polygon.piece, &plane)? {
                Clipped::Front => {
                    above.0.push(polygon.clone());
                    above.1.push(*source);
                }
                Clipped::Back => {
                    below.0.push(polygon.clone());
                    below.1.push(*source);
                }
                Clipped::Split(front, back) => {
                    above.0.push(part(front));
                    above.1.push(*source);
                    below.0.push(part(back));
                    below.1.push(*source);
                }
                Clipped::On => {
                    in_plane = true;
                    break;
                }
            }
        }
        if in_plane || below.0.len().saturating_add(above.0.len()) >= enough {
            continue;
        }
        let reference =
            reference_above(&node.cell, axis, at, &node.reference, &node.polygons, meter)?;
        let depth = node.depth.saturating_add(1);
        let moved = |corner: [i64; 3]| {
            let mut i = 0..;
            corner.map(|v| if i.next() == Some(axis) { at } else { v })
        };
        let under = Cell {
            low: node.cell.low,
            high: moved(node.cell.high),
        };
        let over = Cell {
            low: moved(node.cell.low),
            high: node.cell.high,
        };
        let bit = 1u128
            .checked_shl(127u32.saturating_sub(node.depth))
            .unwrap_or(0);
        return Ok(Outcome::Split(Box::new((
            Node {
                key: node.key,
                depth,
                cell: under,
                reference: node.reference.clone(),
                polygons: below.0,
                sources: below.1,
            },
            Node {
                key: node.key | bit,
                depth,
                cell: over,
                reference,
                polygons: above.0,
                sources: above.1,
            },
        ))));
    }
    Ok(Outcome::Leaf)
}

/// A leaf's pieces, each naming its polygon in the input.
fn solve(node: &Node, meter: &mut WorkMeter<'_>) -> Result<Vec<Classified>, TraceError> {
    let pieces: Vec<Piece> = node.polygons.iter().map(|p| p.piece.clone()).collect();
    let cuts = local_cuts(&pieces, meter)?;
    let mut classified = classify(&node.cell, &node.reference, &node.polygons, &cuts, meter)?;
    for piece in &mut classified {
        piece.polygon = node
            .sources
            .get(piece.polygon)
            .copied()
            .unwrap_or(piece.polygon);
    }
    Ok(classified)
}

/// Every piece of `polygons` with its vectors, in tree order, from the box `cell` whose reference
/// vector is `reference`. A box holds at most `leaf` polygons unless splits no longer separate
/// them, [`LEAF_POLYGONS`] as a rule. With `tests`, a box they all settle is dropped.
///
/// # Errors
///
/// A [`RunError`] of a [`TraceError`]: the lowest-numbered failure of the level or the leaves, or a
/// budget that ran out.
pub fn arrange(
    exec: &Exec<'_>,
    cell: Cell,
    reference: Winding,
    polygons: &[Polygon],
    leaf: usize,
    tests: &[WindingTest],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Classified>, RunError<TraceError>> {
    let mut level = alloc::vec![Node {
        key: 0,
        depth: 0,
        cell,
        reference,
        polygons: polygons.to_vec(),
        sources: (0..polygons.len()).collect(),
    }];
    let mut leaves = Vec::new();
    while !level.is_empty() {
        let outcomes = exec.try_map(meter, SPLITS, ONE_BOX, &level, |_, node, m| {
            divide(node, leaf, tests, m)
        })?;
        let mut next = Vec::new();
        for (node, outcome) in level.into_iter().zip(outcomes) {
            match outcome {
                Outcome::Dropped => {}
                Outcome::Leaf => leaves.push(node),
                Outcome::Split(children) => {
                    let (under, over) = *children;
                    next.push(under);
                    next.push(over);
                }
            }
        }
        level = next;
    }
    leaves.sort_unstable_by_key(|node| node.key);
    let solved = exec.try_map(meter, LEAVES, ONE_BOX, &leaves, |_, node, m| solve(node, m))?;
    Ok(solved.into_iter().flatten().collect())
}
