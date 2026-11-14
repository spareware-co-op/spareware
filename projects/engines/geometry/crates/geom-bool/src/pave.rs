//! Paves, pave blocks and split edges: where things touch an edge, in an order that is proven (11.4
//! step 3, G243).
//!
//! What it does: a [`Pave`] is a vertex of the arrangement on an edge, with the enclosure of its
//! parameter there. [`order`] sorts an edge's paves and proves their order: two paves of
//! different vertices whose enclosures meet fail with [`OrderError::Undecided`], and two paves of
//! one vertex whose enclosures meet are one pave, at the meet of the two. [`blocks`] cuts an edge
//! into [`PaveBlock`]s between neighbouring paves; a closed edge's last block runs round to its
//! first pave, and a closed edge with no pave is one block with no ends. [`split`] gives each block
//! as a [`SplitEdge`] with its history: the edge it comes from, and whether it is that edge
//! unchanged or a piece of it.
//!
//! Guarantees:
//!
//! - **An order is proven or refused.** Neighbouring paves of different vertices are kept only when
//!   the first enclosure ends strictly before the second begins; otherwise the edge fails with
//!   [`OrderError::Undecided`], naming both vertices and their enclosures. No two parameters are
//!   compared within a tolerance.
//! - **One vertex is one pave where it touches the edge once.** Paves of one vertex whose
//!   enclosures meet become one, whose enclosure is the meet of theirs; a vertex met twice at
//!   enclosures proven apart (an edge that leaves a vertex and comes back to it) keeps both.
//! - **The blocks cover the edge.** Each block runs from a pave to the next; an open edge's blocks
//!   run from its first pave to its last, and a closed edge's from each pave round to the next.
//! - **Deterministic.** Paves are sorted by the lower end of their enclosure, then its upper end,
//!   then the vertex index, so the result does not depend on the order they were found in.
//!
//! Method: chapter 11.4, step 3 (`projects/engines/geometry/architecture/11-booleans.md`): the
//! paves, pave blocks and split edges of `OpenCascade`'s General Fuse, studied for method only and
//! described in the evidence note (`projects/kit/notes/G01-evidence/02-occt-parasolid-acis.md`),
//! with each parameter an enclosure and their order proven instead of compared within a tolerance.
//!
//! Cost: O(n log n) in an edge's paves for the order, O(n) for the blocks and the split edges. No
//! recursion.

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;

use geom_num::NumError;
use geom_num::interval::Interval;

/// A vertex of the arrangement on an edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pave {
    /// The vertex, by its index in the arrangement's vertices.
    pub vertex: usize,
    /// The enclosure of the vertex's parameter on the edge.
    pub t: Interval,
}

/// Two paves of an edge whose order cannot be proven.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaveError {
    /// The edge.
    pub edge: usize,
    /// The first pave, by the lower end of its enclosure.
    pub first: Pave,
    /// The pave after it, whose enclosure meets the first's.
    pub second: Pave,
}

impl fmt::Display for PaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the order of vertices {} and {} on edge {} is not proven: their parameters meet",
            self.first.vertex, self.second.vertex, self.edge
        )
    }
}

impl core::error::Error for PaveError {}

/// The order of paves along an edge: by the lower end of the enclosure, then its upper end, then
/// the vertex.
fn along(a: &Pave, b: &Pave) -> Ordering {
    a.t.lo()
        .total_cmp(&b.t.lo())
        .then_with(|| a.t.hi().total_cmp(&b.t.hi()))
        .then_with(|| a.vertex.cmp(&b.vertex))
}

/// The larger of two doubles that are not NaN.
fn larger(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

/// The smaller of two doubles that are not NaN.
fn smaller(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

/// Why an edge's paves could not be put in order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OrderError {
    /// Two paves of different vertices whose enclosures meet.
    Undecided(PaveError),
    /// An enclosure could not be formed.
    Num(NumError),
}

impl From<NumError> for OrderError {
    fn from(e: NumError) -> Self {
        OrderError::Num(e)
    }
}

/// The paves of `edge` in their proven order, one per meeting of a vertex with the edge.
///
/// # Errors
///
/// [`OrderError::Undecided`] for two paves of different vertices whose enclosures meet;
/// [`OrderError::Num`] for an enclosure that cannot be formed.
pub fn order(edge: usize, mut paves: Vec<Pave>) -> Result<Vec<Pave>, OrderError> {
    paves.sort_by(along);
    let mut out: Vec<Pave> = Vec::with_capacity(paves.len());
    for pave in paves {
        let Some(last) = out.last_mut() else {
            out.push(pave);
            continue;
        };
        if last.t.hi() < pave.t.lo() {
            out.push(pave);
            continue;
        }
        if last.vertex != pave.vertex {
            return Err(OrderError::Undecided(PaveError {
                edge,
                first: *last,
                second: pave,
            }));
        }
        // One vertex met once: the meet of its two enclosures, which is not empty since they meet.
        let lo = larger(last.t.lo(), pave.t.lo());
        let hi = smaller(last.t.hi(), pave.t.hi());
        last.t = Interval::new(lo, hi)?;
    }
    Ok(out)
}

/// A piece of an edge between two neighbouring paves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaveBlock {
    /// The edge, by its index in the arrangement's edges.
    pub edge: usize,
    /// The pave it starts at; `None` only for a closed edge with no pave.
    pub from: Option<Pave>,
    /// The pave it ends at; `None` only for a closed edge with no pave.
    pub to: Option<Pave>,
    /// Whether it runs round a closed edge's seam, from its last pave to its first.
    pub wraps: bool,
}

impl PaveBlock {
    /// Its two end vertices, lower index first, or `None` for a closed edge with no pave.
    #[must_use]
    pub fn ends(&self) -> Option<(usize, usize)> {
        let (from, to) = (self.from?, self.to?);
        Some((from.vertex.min(to.vertex), from.vertex.max(to.vertex)))
    }
}

/// The blocks of `edge`, whose paves are `paves` in their proven order; `closed` for an edge whose
/// curve closes, with no vertex of its own.
#[must_use]
pub fn blocks(edge: usize, paves: &[Pave], closed: bool) -> Vec<PaveBlock> {
    let mut out: Vec<PaveBlock> = paves
        .iter()
        .zip(paves.iter().skip(1))
        .map(|(a, b): (&Pave, &Pave)| PaveBlock {
            edge,
            from: Some(*a),
            to: Some(*b),
            wraps: false,
        })
        .collect();
    if closed {
        match (paves.first(), paves.last()) {
            (Some(first), Some(last)) => out.push(PaveBlock {
                edge,
                from: Some(*last),
                to: Some(*first),
                wraps: true,
            }),
            _ => out.push(PaveBlock {
                edge,
                from: None,
                to: None,
                wraps: false,
            }),
        }
    }
    out
}

/// What became of an edge (chapter 20's history, for the edges of an arrangement).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum History {
    /// The edge is whole: one block from its own start to its own end, or a closed edge untouched.
    Unchanged,
    /// The edge is cut: this is one of its pieces.
    Split,
}

/// One piece of an edge, as an edge of its own, with where it comes from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitEdge {
    /// The block it is.
    pub block: PaveBlock,
    /// The edge it comes from.
    pub source: usize,
    /// Whether it is that edge unchanged or a piece of it.
    pub history: History,
}

/// The split edges of `edge`: one per block, each recording `edge` as its source. The edge is
/// unchanged when nothing touches it inside: an open edge with only the paves of its own two ends,
/// or a closed edge with no pave.
#[must_use]
pub fn split(edge: usize, paves: &[Pave], closed: bool) -> Vec<SplitEdge> {
    let pieces = blocks(edge, paves, closed);
    let whole = if closed {
        paves.is_empty()
    } else {
        paves.len() == 2
    };
    let history = if whole {
        History::Unchanged
    } else {
        History::Split
    };
    pieces
        .into_iter()
        .map(|block: PaveBlock| SplitEdge {
            block,
            source: edge,
            history,
        })
        .collect()
}
