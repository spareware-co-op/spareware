//! Adaptive subdivision and the parallel task tree: steps 1 and 4 of 11.3, around the local work.
//!
//! What it does: [`arrange`] takes the polygons of an arrangement and classifies every piece. A
//! box is split in two along its longest axis until it holds few polygons; polygons across the
//! split are clipped exactly, and the upper box's reference point is walked up from the lower's,
//! adding up the polygons it crosses, so each leaf can be solved on its own: [`local_cuts`], then
//! [`classify`]. A box whose every test is settled by its reference vector alone, whatever the
//! polygons in it do, is dropped (step 4). [`arrange_with`] is also told which entries are solids:
//! no two of their faces cross, and their winding is 1 just behind every face and 0 just in front,
//! EMBER's no-self-intersection and no-nested-parts flags (its section 4.5.1, 11.3 step 4). A box
//! whose polygons all bound one solid entry is a leaf at once, however many it holds, and is
//! solved without cuts or tracing.
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
//! - **A solid's leaf is what tracing would give.** In a leaf whose polygons all bound one solid
//!   entry, no polygon crosses another, so each is one piece; the solid's count is 0 in front of
//!   each and 1 behind, and every other entry's is the reference's, as nothing of theirs is in
//!   the box. Each piece is the first of its plane, as no face of a solid overlaps another. Such a
//!   box is not split, so its pieces are fewer than tracing's leaves give, with the same vectors
//!   at every point (tested against the traced arrangement and the blocks' own counts). The caller
//!   vouches for the flag: a wrong one gives wrong pieces.
//!
//! Method: chapter 11.3's five steps (`projects/engines/geometry/architecture/11-booleans.md`),
//! after EMBER's sections 4.1, 4.2 and 4.5 (Trettner, Nehring-Wirxel and Kobbelt 2022,
//! <https://doi.org/10.1145/3528223.3530181>); Kleene's three-valued logic for the early outs,
//! in [`crate::winding`]; the phases of chapter 23.4.
//!
//! Cost: O(n) clips per level for n polygons across the level's boxes, and at most 123 levels:
//! a box of the frame halves 41 times along each axis. Each leaf costs what [`local_cuts`] and
//! [`classify`] cost on its polygons, and a solid's leaf one copy of each. A polygon's corner points
//! are made once and carried into every box it enters whole; only a split's two parts make theirs.
//! Charges the meter one unit per polygon a split clips, and one per polygon of a solid's leaf.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Reverse;

use geom_num::grid::Coord;
use geom_num::plane::{Plane, Tpi};
use geom_num::{NumError, Sign};
use geom_run::{ChunkPlan, Exec, Phase, RunError, WorkMeter};

use crate::convex::Piece;
use crate::cut::{Clipped, clip_at, local_cuts_at, meetings};
use crate::order::Meeting;
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
    /// Each polygon's corner points, as `crate::cut::meetings` makes them: made once, and kept
    /// by every box a polygon passes into whole.
    points: Vec<Vec<Option<Meeting>>>,
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
            let coordinates = nearest(face, bound, next)?;
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

/// The grid point nearest where `face`, `bound` and `next` meet, as `Tpi::to_grid` gives it, in
/// machine words where the point fits them, and refused as it refuses.
fn nearest(face: &Plane, bound: &Plane, next: &Plane) -> Result<[i64; 3], NumError> {
    let words = Meeting::new(face, bound, next)
        .filter(|point| point.weight() != Sign::Zero)
        .and_then(|point| point.rounded());
    if let Some([x, y, z]) = words {
        return Ok([
            Coord::from_i128(x)?.get(),
            Coord::from_i128(y)?.get(),
            Coord::from_i128(z)?.get(),
        ]);
    }
    let at = Tpi::meeting(*face, *bound, *next)?.to_grid()?.value();
    Ok([at.x.get(), at.y.get(), at.z.get()])
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

/// The solid entry every polygon of `node` bounds, if there is one.
fn one_solid(node: &Node, solids: &[Entry]) -> Option<Entry> {
    let only = node.polygons.first().and_then(|p| p.entry)?;
    (solids.contains(&only) && node.polygons.iter().all(|p| p.entry == Some(only))).then_some(only)
}

/// What this level does to `node`. A box whose polygons all bound one solid is a leaf at once:
/// solving it costs one copy of each polygon, so splitting it only makes more pieces to merge.
fn divide(
    node: &Node,
    (leaf, solids): (usize, &[Entry]),
    tests: &[WindingTest],
    meter: &mut WorkMeter<'_>,
) -> Result<Outcome, TraceError> {
    if settled(node, tests) {
        return Ok(Outcome::Dropped);
    }
    let count = node.polygons.len();
    if count <= leaf || one_solid(node, solids).is_some() {
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
        let mut below = (Vec::new(), Vec::new(), Vec::new());
        let mut above = (Vec::new(), Vec::new(), Vec::new());
        let mut in_plane = false;
        let each = node.polygons.iter().zip(&node.sources).zip(&node.points);
        for ((polygon, source), points) in each {
            meter.charge(1)?;
            let part = |piece: Piece| Polygon {
                piece,
                entry: polygon.entry,
            };
            match clip_at(&polygon.piece, points, &plane)? {
                Clipped::Front => {
                    above.0.push(polygon.clone());
                    above.1.push(*source);
                    above.2.push(points.clone());
                }
                Clipped::Back => {
                    below.0.push(polygon.clone());
                    below.1.push(*source);
                    below.2.push(points.clone());
                }
                Clipped::Split(front, back) => {
                    above.2.push(meetings(&front));
                    above.0.push(part(front));
                    above.1.push(*source);
                    below.2.push(meetings(&back));
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
                points: below.2,
            },
            Node {
                key: node.key | bit,
                depth,
                cell: over,
                reference,
                polygons: above.0,
                sources: above.1,
                points: above.2,
            },
        ))));
    }
    Ok(Outcome::Leaf)
}

/// A leaf's pieces, each naming its polygon in the input.
fn solve(
    node: &Node,
    solids: &[Entry],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Classified>, TraceError> {
    if let Some(entry) = one_solid(node, solids) {
        return solid_leaf(node, entry, meter);
    }
    let pieces: Vec<Piece> = node.polygons.iter().map(|p| p.piece.clone()).collect();
    let cuts = local_cuts_at(&pieces, &node.points, meter)?;
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

/// The pieces of a leaf whose polygons all bound the solid `entry`: each polygon whole, with the
/// reference's vector, the solid's count 0 in front and 1 behind.
fn solid_leaf(
    node: &Node,
    entry: Entry,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Classified>, TraceError> {
    let mut front = node.reference.clone();
    front.add(entry, front.count(entry).saturating_neg())?;
    let mut back = front.clone();
    back.add(entry, 1)?;
    let mut out = Vec::with_capacity(node.polygons.len());
    for (at, polygon) in node.polygons.iter().enumerate() {
        meter.charge(1)?;
        out.push(Classified {
            polygon: node.sources.get(at).copied().unwrap_or(at),
            piece: polygon.piece.clone(),
            front: front.clone(),
            back: back.clone(),
            first: true,
        });
    }
    Ok(out)
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
    arrange_with(exec, (cell, reference), polygons, (leaf, tests), &[], meter)
}

/// [`arrange`], told that the entries `solids` are solids: no two of their faces cross, and their
/// winding is 1 just behind every face and 0 just in front. A box whose polygons all bound one of
/// them is a leaf, solved without cuts or tracing: fewer pieces, with the same vectors at every
/// point.
///
/// # Errors
///
/// As [`arrange`].
pub fn arrange_with(
    exec: &Exec<'_>,
    (cell, reference): (Cell, Winding),
    polygons: &[Polygon],
    (leaf, tests): (usize, &[WindingTest]),
    solids: &[Entry],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Classified>, RunError<TraceError>> {
    let mut level = alloc::vec![Node {
        key: 0,
        depth: 0,
        cell,
        reference,
        polygons: polygons.to_vec(),
        sources: (0..polygons.len()).collect(),
        points: polygons.iter().map(|p| meetings(&p.piece)).collect(),
    }];
    let mut leaves = Vec::new();
    while !level.is_empty() {
        let outcomes = exec.try_map(meter, SPLITS, ONE_BOX, &level, |_, node, m| {
            divide(node, (leaf, solids), tests, m)
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
    let solved = exec.try_map(meter, LEAVES, ONE_BOX, &leaves, |_, node, m| {
        solve(node, solids, m)
    })?;
    Ok(solved.into_iter().flatten().collect())
}
