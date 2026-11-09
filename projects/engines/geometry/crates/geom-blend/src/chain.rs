//! Chaining the edges of a blend into tangent-continuous chains (13.10, "Feasibility first", step 1,
//! G262).
//!
//! What it does: the edges a caller wants blended are given as [`EdgeRow`]s, each with the two
//! vertices it joins, and the caller says, at each vertex where two of them meet, how they continue
//! each other ([`Join`]): [`Tangency::Tangent`] when the tangents are certified equal (exact for
//! analytic pairs), [`Tangency::Sharp`] when they are certified apart, and
//! [`Tangency::Undecided`] when the angle between them is inside the band where the ball's path is
//! ill-conditioned. [`chains`] links edges across their tangent joins into [`Chain`]s, each an
//! ordered run of edges with the way each is walked, open or closed. A vertex where more than two
//! of the edges meet, or where the join is sharp, ends the chains that reach it: it is a corner.
//!
//! Guarantees:
//!
//! - **Certified or refused.** An edge continues another only across a [`Tangency::Tangent`] join.
//!   An undecided join is [`ChainError::IllConditioned`] with the vertex and the band, never treated
//!   as smooth; the remedies it offers are to heal the edge to tangent, to leave it out, or to use a
//!   radius that clears the band.
//! - **Every edge in exactly one chain.** The chains partition the given edges, whatever order they
//!   were given in.
//! - **Deterministic.** Chains start from the lowest edge id that is not yet in one, run on from
//!   its start vertex end first, and are returned in the order of their first edges; a closed chain
//!   starts at its lowest edge id and walks that edge forward.
//! - **No guesses at a corner.** Three or more of the edges at a vertex, or a vertex with a sharp
//!   join, end the chains there; a join named for a vertex or an edge that is not given is
//!   [`ChainError::UnknownJoin`].
//!
//! Method: chapter 13.10 of the kernel architecture, "Feasibility first", step 1
//! (`projects/engines/geometry/architecture/13-sweeps-blends-offsets.md`), which takes the
//! near-tangent treatment of the survey of Vida, Martin and Varady, CAD 26(5), 1994
//! (<https://doi.org/10.1016/0010-4485(94)90023-X>), and proves tangency instead of assuming it.
//!
//! Cost: linear in the number of edges and joins, sorted once. No recursion.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;

use geom_num::{NumError, Steps};

/// The identity of an edge, in an order that breaks every tie.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeId(pub u32);

/// The identity of a vertex.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VertexId(pub u32);

/// An edge to blend, with the two vertices it joins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeRow {
    /// The edge.
    pub id: EdgeId,
    /// The vertex it starts at.
    pub start: VertexId,
    /// The vertex it ends at. The same vertex as `start` for an edge that closes on itself.
    pub end: VertexId,
}

/// How two edges meeting at a vertex continue each other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tangency {
    /// Their tangents are certified equal: the chain goes on.
    Tangent,
    /// Their tangents are certified apart: the chain ends.
    Sharp,
    /// The angle is inside the band where the ball's path is ill-conditioned, of this many steps:
    /// neither certified tangent nor certified apart.
    Undecided(Steps),
}

/// Two of the given edges meeting at a vertex.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Join {
    /// The vertex.
    pub at: VertexId,
    /// One edge.
    pub first: EdgeId,
    /// The other.
    pub second: EdgeId,
    /// How they continue each other.
    pub tangency: Tangency,
}

/// An edge in a chain, and the way the chain walks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Link {
    /// The edge.
    pub edge: EdgeId,
    /// Whether the chain walks it from its end to its start.
    pub reversed: bool,
}

/// A run of edges that continue each other tangentially.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chain {
    /// The edges, in the order the chain walks them.
    pub links: Vec<Link>,
    /// Whether the last edge continues the first: the chain is a loop.
    pub closed: bool,
}

/// Why edges could not be chained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainError {
    /// Two edges meet inside the band where tangency cannot be decided: neither certified smooth nor
    /// certified sharp. Heal the edge to tangent, leave it out, or use a radius that clears the band.
    IllConditioned {
        /// The vertex they meet at.
        vertex: VertexId,
        /// The band, in steps.
        band: Steps,
    },
    /// A join names an edge that was not given, or a vertex that neither of its edges has.
    UnknownJoin {
        /// The vertex the join names.
        vertex: VertexId,
    },
    /// An edge is given twice.
    DuplicateEdge(EdgeId),
}

impl fmt::Display for ChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChainError::IllConditioned { .. } => f.write_str(
                "two edges meet in the band where tangency cannot be decided: heal one to tangent, leave it out, or use a radius that clears the band",
            ),
            ChainError::UnknownJoin { .. } => {
                f.write_str("a join names an edge or a vertex that was not given")
            }
            ChainError::DuplicateEdge(_) => f.write_str("an edge is given twice"),
        }
    }
}

impl core::error::Error for ChainError {}

/// The end of an edge a chain reaches: which vertex, and whether that is the edge's start.
fn other_end(row: &EdgeRow, at: VertexId) -> Option<(VertexId, bool)> {
    if row.start == at && row.end == at {
        Some((row.start, true))
    } else if row.start == at {
        Some((row.end, true))
    } else if row.end == at {
        Some((row.start, false))
    } else {
        None
    }
}

/// The edges by id, and where each tangent continuation goes: (edge, vertex) to the next edge.
type Table = (
    BTreeMap<EdgeId, EdgeRow>,
    BTreeMap<(EdgeId, VertexId), EdgeId>,
);

/// Reads the edges and the joins into the table of continuations.
fn table(edges: &[EdgeRow], joins: &[Join]) -> Result<Table, ChainError> {
    let mut rows: BTreeMap<EdgeId, EdgeRow> = BTreeMap::new();
    for row in edges {
        if rows.insert(row.id, *row).is_some() {
            return Err(ChainError::DuplicateEdge(row.id));
        }
    }
    // The edges at each vertex.
    let mut at_vertex: BTreeMap<VertexId, Vec<EdgeId>> = BTreeMap::new();
    for row in rows.values() {
        at_vertex.entry(row.start).or_default().push(row.id);
        if row.end != row.start {
            at_vertex.entry(row.end).or_default().push(row.id);
        }
    }
    // The tangent continuations: edge to (vertex, other edge), over joins that are certified and
    // that meet at a vertex where exactly these two edges are.
    let mut onward: BTreeMap<(EdgeId, VertexId), EdgeId> = BTreeMap::new();
    let mut undecided: Option<(VertexId, Steps)> = None;
    for join in joins {
        let (Some(a), Some(b)) = (rows.get(&join.first), rows.get(&join.second)) else {
            return Err(ChainError::UnknownJoin { vertex: join.at });
        };
        if other_end(a, join.at).is_none() || other_end(b, join.at).is_none() {
            return Err(ChainError::UnknownJoin { vertex: join.at });
        }
        match join.tangency {
            Tangency::Sharp => {}
            Tangency::Undecided(band) => {
                if undecided.is_none_or(|(v, _)| join.at < v) {
                    undecided = Some((join.at, band));
                }
            }
            Tangency::Tangent => {
                let meeting = at_vertex.get(&join.at).map_or(0, Vec::len);
                let open = a.start != a.end && b.start != b.end;
                if meeting == 2 && join.first != join.second && open {
                    onward.insert((join.first, join.at), join.second);
                    onward.insert((join.second, join.at), join.first);
                }
            }
        }
    }
    if let Some((vertex, band)) = undecided {
        return Err(ChainError::IllConditioned { vertex, band });
    }
    Ok((rows, onward))
}

/// Links the edges across their tangent joins into chains.
///
/// # Errors
///
/// [`ChainError::IllConditioned`] for the lowest-numbered vertex whose join is undecided;
/// [`ChainError::UnknownJoin`] for a join that does not match its edges;
/// [`ChainError::DuplicateEdge`] for an edge id given twice.
pub fn chains(edges: &[EdgeRow], joins: &[Join]) -> Result<Vec<Chain>, ChainError> {
    let (rows, onward) = table(edges, joins)?;
    let mut taken: BTreeMap<EdgeId, bool> = rows.keys().map(|id| (*id, false)).collect();
    let ids: Vec<EdgeId> = rows.keys().copied().collect();
    let mut out: Vec<Chain> = Vec::new();
    for &id in &ids {
        if taken.get(&id).copied().unwrap_or(true) {
            continue;
        }
        // Back up from this edge to the first edge of its chain, or find it is a loop.
        let mut head = Link {
            edge: id,
            reversed: false,
        };
        let mut closed = false;
        for _ in 0..=ids.len() {
            let Some(row) = rows.get(&head.edge) else {
                break;
            };
            let entry = if head.reversed { row.end } else { row.start };
            let Some(&previous) = onward.get(&(head.edge, entry)) else {
                break;
            };
            if previous == id {
                closed = true;
                break;
            }
            let Some(prow) = rows.get(&previous) else {
                break;
            };
            head = Link {
                edge: previous,
                reversed: prow.start == entry && prow.end != entry,
            };
        }
        if closed {
            head = Link {
                edge: id,
                reversed: false,
            };
        }
        // Walk forward from it.
        let mut links = alloc::vec![head];
        if let Some(t) = taken.get_mut(&head.edge) {
            *t = true;
        }
        let mut current = head;
        for _ in 0..=ids.len() {
            let Some(row) = rows.get(&current.edge) else {
                break;
            };
            let exit = if current.reversed { row.start } else { row.end };
            let Some(&next) = onward.get(&(current.edge, exit)) else {
                break;
            };
            if next == head.edge {
                closed = true;
                break;
            }
            if taken.get(&next).copied().unwrap_or(true) {
                break;
            }
            let Some(nrow) = rows.get(&next) else {
                break;
            };
            let link = Link {
                edge: next,
                reversed: nrow.end == exit && nrow.start != exit,
            };
            if let Some(t) = taken.get_mut(&next) {
                *t = true;
            }
            links.push(link);
            current = link;
        }
        out.push(Chain { links, closed });
    }
    Ok(out)
}

/// How two straight edges continue each other at a vertex, exactly: `arriving` is the direction
/// the first runs in as it reaches the vertex, `leaving` the direction the second runs in as it
/// leaves it (each a difference of grid points, below 2^41 in size). They continue each other
/// tangentially when they are parallel and point the same way; a hairpin, parallel and opposite, is
/// sharp, and so is every other pair.
///
/// Bit budget: a component of the cross product is below 2^83 and the dot product below 2^84, in
/// `i128`.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] for directions past those sizes.
pub fn straight_tangency(arriving: [i128; 3], leaving: [i128; 3]) -> Result<Tangency, NumError> {
    let [a0, a1, a2] = arriving;
    let [b0, b1, b2] = leaving;
    let part = |p: i128, q: i128, r: i128, s: i128| -> Result<i128, NumError> {
        p.checked_mul(q)
            .and_then(|x| x.checked_sub(r.checked_mul(s)?))
            .ok_or(NumError::BudgetExceeded)
    };
    let cross = [
        part(a1, b2, a2, b1)?,
        part(a2, b0, a0, b2)?,
        part(a0, b1, a1, b0)?,
    ];
    let dot = a0
        .checked_mul(b0)
        .and_then(|x| x.checked_add(a1.checked_mul(b1)?))
        .and_then(|x| x.checked_add(a2.checked_mul(b2)?))
        .ok_or(NumError::BudgetExceeded)?;
    Ok(if cross == [0, 0, 0] && dot > 0 {
        Tangency::Tangent
    } else {
        Tangency::Sharp
    })
}
