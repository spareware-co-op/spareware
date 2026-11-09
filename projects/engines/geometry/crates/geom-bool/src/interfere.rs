//! Interferences, lowest dimension first: where the vertices, edges and faces of the operands meet,
//! as paves on the edges and as each face's information (11.4 steps 2 to 4, G243 and G243a).
//!
//! What it does: [`interfere`] takes an [`Arrangement`] (the carriers, and the vertices, edges and
//! faces of every operand, each edge a bounded curve of the carriers' pool between two of the
//! vertices, or closed, and each face a region of a carrier bounded by loops of edges) and finds
//! where they meet, one dimension at a time. Vertex with vertex: the merge stage
//! ([`crate::merge::merge_clusters`]) joins the vertices of different operands that touch, and
//! every later decision uses the merged vertex. Vertex with edge: a vertex of one operand that
//! touches an edge of another, by the contact rule on its certified distance from the edge's curve,
//! becomes a pave of that edge, at the enclosure of its parameter there. Edge with edge: where two
//! edges of different operands meet (`geom_intersect::curve::curves`), a point that touches a pave
//! already on either edge is that pave's vertex, so a meeting through an existing corner ends at
//! the corner and makes no second vertex nearby; any other point is a new vertex, certified against
//! both curves. Each edge's paves are then put in their proven order ([`crate::pave::order`]), the
//! edges are cut at them into split edges with their history ([`crate::pave::split`]), and the
//! pieces of different edges that are one stretch of curve are found as common blocks
//! ([`crate::common::common_blocks`]). A region where two edges could not be told apart by their
//! intersection, as along a circle and a NURBS close to it, is accepted only where a common block
//! proven by the contact rule covers it on both edges, or where it touches a vertex both edges pass
//! through; anywhere else it fails. Between the edges' meetings and their splitting, the faces meet
//! the vertices and edges, and after it each other ([`crate::face`]): vertex with face, edge with
//! face, face with face, and each face's On, In and section groups.
//!
//! Guarantees:
//!
//! - **Lowest dimension first.** Vertices are merged before any vertex is put on an edge, and every
//!   vertex is put on the edges it touches before any two edges are intersected; a result found at
//!   a lower dimension is reused above it, never found again nearby.
//! - **Never a guess.** A vertex touches an edge only by the contact rule; a pair the rule cannot
//!   decide fails with [`crate::rule::NearContact`], a region an intersection could not decide fails
//!   with [`BuildError::Unresolved`], and two paves whose order is not proven fail with
//!   [`BuildError::Order`].
//! - **New vertices are certified against their carriers.** A vertex made where two edges cross
//!   is the grid point nearest the meeting, proven within its bound of both curves, and refused
//!   with [`crate::rule::CoincidenceChain`] when that bound passes the remaining one.
//! - **Deterministic.** Pairs are taken in index order, each edge's paves are ordered by their
//!   enclosures, and new vertices are numbered in the order their pairs are taken.
//!
//! Method: chapter 11.4, steps 1 to 3 (`projects/engines/geometry/architecture/11-booleans.md`): the
//! interferences of `OpenCascade`'s General Fuse, studied for method only
//! (`projects/kit/notes/G01-evidence/02-occt-parasolid-acis.md`), decided on carriers by the
//! contact rule. The rule that the lowest-dimension result is taken first, and that the end of a
//! meeting is put on an existing vertex rather than a new one, is that of `SMLib`'s topology
//! intersector, read in solidcore's Rust port.
//!
//! Consulted: solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/45d6111b909487e94edaec309a7673a3b73d4ae5/crates/solidcore/src/boolean/topology_intersector/p1.rs>,
//!   for the order of the vertex, edge and face intersections and the lowest-dimension solution;
//!   its tolerance growth on relating entities is not taken. No code copied.
//! SPDX-License-Identifier: Apache-2.0
//!
//! Cost: a box tree over the vertices and edges, then for each candidate pair one certified
//! distance (vertex with edge) or one curve intersection (edge with edge), all charged to the
//! meter; each edge's paves sorted once; and what the face stages cost ([`crate::face`]). No
//! recursion.

use alloc::vec::Vec;
use core::fmt;

use geom_bvh::{Aabb, Entry};
use geom_intersect::curve::curves;
use geom_intersect::project::{Projection, project_curve};
use geom_intersect::solve::Reason;
use geom_intersect::{IntersectError, Item, ParamBox, Unresolved};
use geom_num::interval::Interval;
use geom_num::math::round_ties_even;
use geom_num::{NumError, P3};
use geom_run::{Exec, WorkMeter};
use geom_topo::{Box3, CurveIx, ParamCurve};

use crate::broad::{candidate_pairs, point_entries};
use crate::carrier::{Bound, CarrierIx, PointGeom, SortedVec};
use crate::common::{CommonBlock, common_blocks};
use crate::distance::{Carriers, FIRST_FLOOR};
use crate::face::{self, FaceInfo, SectionEdge};
use crate::merge::merge_clusters;
use crate::pave::{OrderError, Pave, PaveBlock, PaveError, SplitEdge, order, split};
use crate::rule::{
    CoincidenceChain, Contact, ContactError, EPS_FIT, NearContact, REFINEMENTS, Verdict, decide,
    judge,
};

/// A vertex of an operand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexIn {
    /// Where it is.
    pub point: PointGeom,
    /// The operand it belongs to.
    pub operand: u32,
}

/// An edge of an operand: the whole of a curve of the carriers' pool, whose domain is the edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeIn {
    /// Its curve, whose domain runs from the edge's start to its end.
    pub curve: CurveIx,
    /// Its start and end vertices, by index; `None` for a closed edge.
    pub ends: Option<[usize; 2]>,
    /// The operand it belongs to.
    pub operand: u32,
}

/// A face of an operand: a region of its carrier bounded by loops of the arrangement's edges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaceIn {
    /// Its carrier: a plane of the table for a flat face, a surface of the pool otherwise.
    pub surface: CarrierIx,
    /// Its loops, each the indices of its edges in order round it, the outer loop first.
    pub loops: Vec<Vec<usize>>,
    /// The operand it belongs to.
    pub operand: u32,
}

/// What the interferences are found in: the carriers, and the vertices, edges and faces of every
/// operand.
#[derive(Debug, Default)]
pub struct Arrangement {
    /// The carriers every vertex, edge and face is defined on.
    pub carriers: Carriers,
    /// The vertices.
    pub vertices: Vec<VertexIn>,
    /// The edges.
    pub edges: Vec<EdgeIn>,
    /// The faces.
    pub faces: Vec<FaceIn>,
}

/// An entity of the arrangement, by kind and index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Entity {
    /// A vertex, by its index among the merged vertices.
    Vertex(usize),
    /// An edge.
    Edge(usize),
    /// A face.
    Face(usize),
}

/// Why the interferences could not be found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BuildError {
    /// A decision of the contact rule, or a merge, could not be taken.
    Contact(ContactError),
    /// Two paves of an edge whose order is not proven.
    Order(PaveError),
    /// An intersection left a region it could not decide.
    Unresolved {
        /// The first entity.
        first: Entity,
        /// The second.
        second: Entity,
        /// Why it is undecided.
        why: Unresolved,
        /// Where, in space.
        at: Box3,
    },
    /// A vertex's nearest points on an edge are not isolated: it stands on the axis of a round
    /// edge, so it has no one parameter there.
    NotIsolated {
        /// The vertex.
        vertex: usize,
        /// The edge.
        edge: usize,
    },
    /// An input refers to a vertex, curve, face or plane that is not there.
    Missing(Entity),
    /// A region where an edge meets a face's plane that the crossing engine could not decide, and
    /// that no vertex the two share covers.
    Undecided {
        /// The edge.
        edge: usize,
        /// The face.
        face: usize,
        /// Why it is undecided.
        why: Reason,
        /// Where, in space.
        at: Box3,
    },
    /// No ray from a point qualified to place it in a flat face: its picture in the face's plane
    /// lies on the boundary's, though in space it is proven apart from the boundary.
    Unclassified {
        /// The face.
        face: usize,
        /// The point.
        at: P3,
    },
    /// Two paves of a section line whose order is not proven.
    SectionOrder {
        /// The two faces whose planes meet in the line.
        faces: [usize; 2],
        /// The first pave, by the lower end of its enclosure.
        first: Pave,
        /// The pave after it, whose enclosure meets the first's.
        second: Pave,
    },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::Contact(e) => e.fmt(f),
            BuildError::Order(e) => e.fmt(f),
            BuildError::Unresolved { first, second, .. } => write!(
                f,
                "the meeting of {first:?} and {second:?} has a region its intersection could not decide"
            ),
            BuildError::NotIsolated { vertex, edge } => write!(
                f,
                "vertex {vertex} has no one nearest point on edge {edge}: it stands on its axis"
            ),
            BuildError::Missing(e) => write!(f, "{e:?} refers to something that is not there"),
            BuildError::Undecided { edge, face, .. } => write!(
                f,
                "where edge {edge} meets the plane of face {face} has a region the crossing engine could not decide"
            ),
            BuildError::Unclassified { face, .. } => write!(
                f,
                "no ray qualified to place a point in face {face}: its picture lies on the boundary's"
            ),
            BuildError::SectionOrder {
                faces: [a, b],
                first,
                second,
            } => write!(
                f,
                "the order of vertices {} and {} on the line where faces {a} and {b} meet is not proven",
                first.vertex, second.vertex
            ),
        }
    }
}

impl core::error::Error for BuildError {}

impl From<ContactError> for BuildError {
    fn from(e: ContactError) -> Self {
        BuildError::Contact(e)
    }
}

impl From<IntersectError> for BuildError {
    fn from(e: IntersectError) -> Self {
        BuildError::Contact(ContactError::Intersect(e))
    }
}

impl From<NumError> for BuildError {
    fn from(e: NumError) -> Self {
        BuildError::Contact(ContactError::Num(e))
    }
}

impl From<OrderError> for BuildError {
    fn from(e: OrderError) -> Self {
        match e {
            OrderError::Undecided(p) => BuildError::Order(p),
            OrderError::Num(n) => n.into(),
        }
    }
}

/// One meeting found, in the order found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interference {
    /// Input vertices of different operands that touch became one merged vertex.
    VertexVertex {
        /// The input vertex.
        input: usize,
        /// The merged vertex it became.
        vertex: usize,
    },
    /// A vertex touches an edge of another operand.
    VertexEdge {
        /// The merged vertex.
        vertex: usize,
        /// The edge.
        edge: usize,
    },
    /// Two edges of different operands meet at a vertex.
    EdgeEdge {
        /// The edge of lower index.
        first: usize,
        /// The other.
        second: usize,
        /// The vertex they meet at.
        vertex: usize,
        /// Whether the vertex was already there (found at a lower dimension, or by an earlier pair)
        /// rather than made for this meeting.
        reused: bool,
    },
    /// A vertex lies inside a flat face of another operand.
    VertexFace {
        /// The vertex.
        vertex: usize,
        /// The face.
        face: usize,
    },
    /// An edge crosses a flat face of another operand, inside it or on its boundary.
    EdgeFace {
        /// The edge.
        edge: usize,
        /// The face.
        face: usize,
        /// The vertex they meet at.
        vertex: usize,
        /// Whether the vertex was already there rather than made for this meeting.
        reused: bool,
    },
    /// Two flat faces of different operands meet along a section edge.
    FaceFace {
        /// The face of lower index.
        first: usize,
        /// The other.
        second: usize,
        /// The section edge, by its index in [`Interfered::sections`].
        section: usize,
    },
}

/// The interferences of an arrangement.
#[derive(Clone, Debug, PartialEq)]
pub struct Interfered {
    /// The vertices: the merged input vertices, then the vertices made where edges meet.
    pub vertices: Vec<PointGeom>,
    /// Each input vertex's index in `vertices`.
    pub merged: Vec<usize>,
    /// Each edge's paves, in their proven order.
    pub paves: Vec<Vec<Pave>>,
    /// Each edge's start and end, as merged vertices; `None` for a closed edge.
    pub ends: Vec<Option<[usize; 2]>>,
    /// What met what, lowest dimension first.
    pub interferences: Vec<Interference>,
    /// Every edge cut at its paves, in edge order, each piece with the edge it comes from.
    pub splits: Vec<SplitEdge>,
    /// The pieces of different edges that are one stretch of curve, by index into `splits`.
    pub common: Vec<CommonBlock>,
    /// Each face's information: its On, In and section groups, and what waits for face splitting.
    pub faces: Vec<FaceInfo>,
    /// The section edges: where the planes of two flat faces meet, kept where inside both.
    pub sections: Vec<SectionEdge>,
}

/// A region two edges' intersection could not decide, held until the common blocks are known.
struct Deferred {
    edges: (usize, usize),
    at: ParamBox,
    xyz: Box3,
    why: Unresolved,
}

/// Whether a parameter enclosure lies within a split edge's span.
fn within_block(t: Interval, split: &SplitEdge) -> bool {
    match (split.block.from, split.block.to) {
        (Some(from), Some(to)) if !split.block.wraps => {
            t.lo() >= from.t.lo() && t.hi() <= to.t.hi()
        }
        _ => false,
    }
}

/// Whether a common block covers a deferred region on both its edges.
fn covered(region: &Deferred, splits: &[SplitEdge], common: &[CommonBlock]) -> bool {
    let (a, b) = region.edges;
    let (Some(s), Some(t)) = (region.at.spans().first(), region.at.spans().get(1)) else {
        return false;
    };
    common.iter().any(|block: &CommonBlock| {
        let holds = |edge: usize, span: Interval| {
            block.members.iter().any(|m: &usize| {
                splits
                    .get(*m)
                    .is_some_and(|piece| piece.source == edge && within_block(span, piece))
            })
        };
        holds(a, *s) && holds(b, *t)
    })
}

/// A grid point's coordinates as doubles, which hold them exactly (at most 2^40).
pub(crate) fn coordinates(p: P3) -> [f64; 3] {
    #[allow(
        clippy::cast_precision_loss,
        reason = "a coordinate is at most 2^40 in size, which a double holds exactly"
    )]
    [p.x.get() as f64, p.y.get() as f64, p.z.get() as f64]
}

/// The whole number of steps nearest `x`, rounding ties to even.
fn nearest_whole(x: f64) -> Result<i64, NumError> {
    let r = round_ties_even(x)?.get();
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a double past the range saturates, and P3::new refuses it"
    )]
    Ok(r as i64)
}

/// The grid point nearest `v`.
pub(crate) fn nearest_point(v: [f64; 3]) -> Result<P3, NumError> {
    let [x, y, z] = v;
    P3::new(nearest_whole(x)?, nearest_whole(y)?, nearest_whole(z)?)
}

/// A whole number of steps at or below `x`, and one at or above it.
pub(crate) fn floor_ceil(x: f64) -> Result<(i64, i64), NumError> {
    let n = nearest_whole(x)?;
    #[allow(
        clippy::cast_precision_loss,
        reason = "n is within a step of x, which is far below 2^53 in size for a box inside the frame"
    )]
    let back = n as f64;
    let lo = if back > x {
        n.checked_sub(1).ok_or(NumError::BudgetExceeded)?
    } else {
        n
    };
    let hi = if back < x {
        n.checked_add(1).ok_or(NumError::BudgetExceeded)?
    } else {
        n
    };
    Ok((lo, hi))
}

/// The box tree entry of an edge: its curve's box over its domain, padded by the contact radius.
pub(crate) fn edge_entry(
    carriers: &Carriers,
    (e, edge): (usize, &EdgeIn),
    key: u64,
    meter: &mut WorkMeter<'_>,
) -> Result<Entry<3>, BuildError> {
    let curve = carriers
        .curve(edge.curve)
        .ok_or(BuildError::Missing(Entity::Edge(e)))?;
    let bounds = curve
        .bound(curve.domain(), meter)
        .map_err(IntersectError::from)?;
    let pad = i64::try_from(EPS_FIT.0).map_err(|_| NumError::BudgetExceeded)?;
    let mut lo = [0_i64; 3];
    let mut hi = [0_i64; 3];
    for (k, (slot_lo, slot_hi)) in lo.iter_mut().zip(hi.iter_mut()).enumerate() {
        let (a, b) = (
            bounds.lo.get(k).copied().ok_or(NumError::Degenerate)?,
            bounds.hi.get(k).copied().ok_or(NumError::Degenerate)?,
        );
        *slot_lo = floor_ceil(a)?
            .0
            .checked_sub(pad)
            .ok_or(NumError::BudgetExceeded)?;
        *slot_hi = floor_ceil(b)?
            .1
            .checked_add(pad)
            .ok_or(NumError::BudgetExceeded)?;
    }
    Ok(Entry {
        key,
        bbox: Aabb::new(lo, hi).map_err(ContactError::from)?,
    })
}

/// The enclosure of a distance widened by a point's own bound each way, and never below zero.
pub(crate) fn widened(d: Interval, by: Bound) -> Result<Interval, NumError> {
    #[allow(
        clippy::cast_precision_loss,
        reason = "bounds are counts of steps far below 2^53"
    )]
    let w = by.steps() as f64;
    let lo = d.lo() - w;
    Interval::new(if lo > 0.0 { lo } else { 0.0 }, d.hi() + w)
}

/// The enclosure of the distance from a grid point, within `w` of where its vertex is, to a box of
/// grid points: the box's nearest and farthest corners, widened by `w`.
pub(crate) fn to_box(p: P3, w: Bound, region: &Box3) -> Result<Interval, NumError> {
    let (lo, hi) = (region.min(), region.max());
    let mut near = [Interval::point(0.0)?; 3];
    let mut far = [Interval::point(0.0)?; 3];
    let axes = [
        (p.x.get(), lo.x.get(), hi.x.get()),
        (p.y.get(), lo.y.get(), hi.y.get()),
        (p.z.get(), lo.z.get(), hi.z.get()),
    ];
    for ((n, f), (c, a, b)) in near.iter_mut().zip(far.iter_mut()).zip(axes) {
        let gap = if c < a {
            a.checked_sub(c)
        } else if c > b {
            c.checked_sub(b)
        } else {
            Some(0)
        }
        .ok_or(NumError::BudgetExceeded)?;
        let reach = c
            .checked_sub(a)
            .map(i64::unsigned_abs)
            .zip(b.checked_sub(c).map(i64::unsigned_abs))
            .map(|(x, y)| x.max(y))
            .ok_or(NumError::BudgetExceeded)?;
        #[allow(
            clippy::cast_precision_loss,
            reason = "differences of coordinates are at most 2^41, which a double holds exactly"
        )]
        let (g, r) = (gap as f64, reach as f64);
        *n = Interval::point(g)?;
        *f = Interval::point(r)?;
    }
    let (dn, df) = (Interval::norm(&near)?, Interval::norm(&far)?);
    widened(Interval::new(dn.lo(), df.hi())?, w)
}

/// The vertex at index `v`, committed: its grid point and its bound.
pub(crate) fn committed(
    carriers: &Carriers,
    vertices: &[PointGeom],
    v: usize,
) -> Result<(P3, Bound), BuildError> {
    let point = vertices
        .get(v)
        .ok_or(BuildError::Missing(Entity::Vertex(v)))?;
    Ok(carriers.commit(point)?)
}

/// The curve of edge `e`.
fn curve_of(arr: &Arrangement, e: usize) -> Result<CurveIx, BuildError> {
    Ok(arr
        .edges
        .get(e)
        .ok_or(BuildError::Missing(Entity::Edge(e)))?
        .curve)
}

/// The parameter of vertex `v`, at grid point `p`, on edge `e`: the enclosure of its nearest
/// point's parameter at the finest refinement.
pub(crate) fn parameter(
    arr: &Arrangement,
    e: usize,
    v: usize,
    p: P3,
    meter: &mut WorkMeter<'_>,
) -> Result<Interval, BuildError> {
    let ix = curve_of(arr, e)?;
    let curve = arr
        .carriers
        .curve(ix)
        .ok_or(BuildError::Missing(Entity::Edge(e)))?;
    let scale = (0..REFINEMENTS).fold(1.0_f64, |s, _| s * 2.0);
    match project_curve(curve, &coordinates(p), FIRST_FLOOR / scale, meter)? {
        Projection::Points(found) => found
            .first()
            .and_then(|n| n.at.first().copied())
            .ok_or(BuildError::NotIsolated { vertex: v, edge: e }),
        Projection::Set { .. } | Projection::NonIsolated { .. } => {
            Err(BuildError::NotIsolated { vertex: v, edge: e })
        }
    }
}

/// The candidate pairs of the broad phase: vertex with edge, and edge with edge, each in index
/// order, of different operands.
struct Candidates {
    vertex_edge: Vec<(usize, usize)>,
    edge_edge: Vec<(usize, usize)>,
}

fn candidates(
    arr: &Arrangement,
    vertices: &[PointGeom],
    owner: &[u32],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Candidates, BuildError> {
    let nv = vertices.len();
    let mut entries = point_entries(&arr.carriers, vertices)?;
    for (e, edge) in arr.edges.iter().enumerate() {
        let key = u64::try_from(nv.checked_add(e).ok_or(NumError::BudgetExceeded)?)
            .map_err(|_| NumError::BudgetExceeded)?;
        entries.push(edge_entry(&arr.carriers, (e, edge), key, meter)?);
    }
    let mut out = Candidates {
        vertex_edge: Vec::new(),
        edge_edge: Vec::new(),
    };
    for (a, b) in candidate_pairs(&entries, exec, meter)? {
        let (a, b) = (
            usize::try_from(a).map_err(|_| NumError::BudgetExceeded)?,
            usize::try_from(b).map_err(|_| NumError::BudgetExceeded)?,
        );
        match (a.checked_sub(nv), b.checked_sub(nv)) {
            // Two vertices: the merge stage has decided them.
            (None, None) => {}
            (None, Some(e)) | (Some(e), None) => {
                let v = if a < nv { a } else { b };
                let edge = arr
                    .edges
                    .get(e)
                    .ok_or(BuildError::Missing(Entity::Edge(e)))?;
                if owner.get(v).is_some_and(|o| *o != edge.operand) {
                    out.vertex_edge.push((v, e));
                }
            }
            (Some(x), Some(y)) => {
                let (ex, ey) = (
                    arr.edges
                        .get(x)
                        .ok_or(BuildError::Missing(Entity::Edge(x)))?,
                    arr.edges
                        .get(y)
                        .ok_or(BuildError::Missing(Entity::Edge(y)))?,
                );
                if ex.operand != ey.operand {
                    out.edge_edge.push((x.min(y), x.max(y)));
                }
            }
        }
    }
    out.vertex_edge.sort_unstable();
    out.edge_edge.sort_unstable();
    Ok(out)
}

/// Whether merged vertex `v` is an end of edge `e`.
fn is_end(ends: &[Option<[usize; 2]>], e: usize, v: usize) -> bool {
    ends.get(e)
        .copied()
        .flatten()
        .is_some_and(|[s, t]| s == v || t == v)
}

/// The vertex among `ids` that a meeting at `xyz` touches, if one does, by the contact rule.
///
/// # Errors
///
/// [`ContactError::NearContact`] for a pave neither proven apart from the meeting nor touching it.
pub(crate) fn touching(
    carriers: &Carriers,
    vertices: &[PointGeom],
    ids: impl IntoIterator<Item = usize>,
    xyz: &Box3,
) -> Result<Option<usize>, BuildError> {
    let (lo, hi) = (xyz.min(), xyz.max());
    for vertex in ids {
        let (p, w) = committed(carriers, vertices, vertex)?;
        // Further than its bound and the contact radius from the box along one axis: proven apart,
        // as the enclosure below would say, without it.
        let pad = i64::try_from(w.steps().saturating_add(EPS_FIT.0)).unwrap_or(i64::MAX);
        let outside =
            |c: i64, a: i64, b: i64| c < a.saturating_sub(pad) || c > b.saturating_add(pad);
        if outside(p.x.get(), lo.x.get(), hi.x.get())
            || outside(p.y.get(), lo.y.get(), hi.y.get())
            || outside(p.z.get(), lo.z.get(), hi.z.get())
        {
            continue;
        }
        let d = to_box(p, w, xyz)?;
        match judge(d) {
            Verdict::Touch => return Ok(Some(vertex)),
            Verdict::Decided(_) => {}
            Verdict::Near => {
                return Err(ContactError::NearContact(NearContact {
                    enclosure: d,
                    refinement: REFINEMENTS,
                })
                .into());
            }
        }
    }
    Ok(None)
}

/// A new vertex where two curves meet within `xyz`: the grid point nearest the box's centre, proven
/// within its bound of both curves.
fn new_vertex(
    carriers: &Carriers,
    curves_met: [CurveIx; 2],
    xyz: &Box3,
    remaining: Bound,
    meter: &mut WorkMeter<'_>,
) -> Result<PointGeom, BuildError> {
    let ([x0, y0, z0], [x1, y1, z1]) = (coordinates(xyz.min()), coordinates(xyz.max()));
    let at = nearest_point([
        x0 / 2.0 + x1 / 2.0,
        y0 / 2.0 + y1 / 2.0,
        z0 / 2.0 + z1 / 2.0,
    ])?;
    let on = SortedVec::from_items(curves_met.iter().map(|c| CarrierIx::Curve(*c)));
    certify(carriers, at, on, remaining, meter)
}

/// The grid point `at` as a vertex on the carriers `on`, proven within its bound of each.
///
/// # Errors
///
/// [`ContactError::CoincidenceChain`] when a bound passes `remaining`; a distance's errors.
pub(crate) fn certify(
    carriers: &Carriers,
    at: P3,
    on: SortedVec<CarrierIx>,
    remaining: Bound,
    meter: &mut WorkMeter<'_>,
) -> Result<PointGeom, BuildError> {
    let mut within = Bound::ZERO;
    for &c in on.as_slice() {
        let (bound, d) = carriers.upper_bound(at, c, REFINEMENTS, meter)?;
        if bound > remaining {
            return Err(ContactError::CoincidenceChain(CoincidenceChain {
                carrier: c,
                distance: d,
                remaining,
            })
            .into());
        }
        if bound > within {
            within = bound;
        }
    }
    Ok(PointGeom::Certified { at, on, within })
}

/// The working state of the stages: the vertices so far, each edge's own ends and its paves so far,
/// and what met what.
pub(crate) struct Stages {
    pub(crate) vertices: Vec<PointGeom>,
    pub(crate) merged: Vec<usize>,
    pub(crate) owner: Vec<u32>,
    pub(crate) ends: Vec<Option<[usize; 2]>>,
    pub(crate) paves: Vec<Vec<Pave>>,
    pub(crate) interferences: Vec<Interference>,
    deferred: Vec<Deferred>,
}

/// Vertex with vertex: the merge stage over pairs of different operands, and each edge's own ends
/// as its first paves.
fn merge_vertices(
    arr: &Arrangement,
    remaining: Bound,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Stages, BuildError> {
    let carriers = &arr.carriers;
    let points: Vec<PointGeom> = arr.vertices.iter().map(|v| v.point.clone()).collect();
    let mut pairs = Vec::new();
    for (a, b) in candidate_pairs(&point_entries(carriers, &points)?, exec, meter)? {
        let (a, b) = (
            usize::try_from(a).map_err(|_| NumError::BudgetExceeded)?,
            usize::try_from(b).map_err(|_| NumError::BudgetExceeded)?,
        );
        let operands = (arr.vertices.get(a), arr.vertices.get(b));
        match operands {
            (Some(va), Some(vb)) if va.operand != vb.operand => pairs.push((a, b)),
            _ => {}
        }
    }
    let (vertices, merged) = merge_clusters(carriers, &points, &pairs, remaining, meter)?;
    let mut interferences = Vec::new();
    // A merged vertex's operand, for the broad phase: the lowest operand among its members. A
    // merged vertex of several operands is on no edge of them but its own, so any one serves.
    let mut owner: Vec<u32> = alloc::vec![u32::MAX; vertices.len()];
    let mut members: Vec<usize> = alloc::vec![0; vertices.len()];
    for (&vertex, v) in merged.iter().zip(&arr.vertices) {
        if let Some(slot) = owner.get_mut(vertex) {
            *slot = (*slot).min(v.operand);
        }
        if let Some(count) = members.get_mut(vertex) {
            *count = count.saturating_add(1);
        }
    }
    for (input, &vertex) in merged.iter().enumerate() {
        if members.get(vertex).is_some_and(|n| *n > 1) {
            interferences.push(Interference::VertexVertex { input, vertex });
        }
    }
    let mut ends: Vec<Option<[usize; 2]>> = Vec::with_capacity(arr.edges.len());
    let mut paves: Vec<Vec<Pave>> = Vec::with_capacity(arr.edges.len());
    for (e, edge) in arr.edges.iter().enumerate() {
        let curve = carriers
            .curve(edge.curve)
            .ok_or(BuildError::Missing(Entity::Edge(e)))?;
        let (t0, t1) = curve.domain();
        let Some([s, t]) = edge.ends else {
            ends.push(None);
            paves.push(Vec::new());
            continue;
        };
        let at = |v: usize| -> Result<usize, BuildError> {
            merged
                .get(v)
                .copied()
                .ok_or(BuildError::Missing(Entity::Vertex(v)))
        };
        let (ms, mt) = (at(s)?, at(t)?);
        ends.push(Some([ms, mt]));
        paves.push(alloc::vec![
            Pave {
                vertex: ms,
                t: Interval::point(t0)?,
            },
            Pave {
                vertex: mt,
                t: Interval::point(t1)?,
            },
        ]);
    }
    Ok(Stages {
        vertices,
        merged,
        owner,
        ends,
        paves,
        interferences,
        deferred: Vec::new(),
    })
}

/// Vertex with edge: each vertex that touches an edge of another operand, by the contact rule,
/// becomes a pave of the edge.
fn vertex_edge(
    arr: &Arrangement,
    st: &mut Stages,
    pairs: &[(usize, usize)],
    meter: &mut WorkMeter<'_>,
) -> Result<(), BuildError> {
    let carriers = &arr.carriers;
    for &(v, e) in pairs {
        if is_end(&st.ends, e, v) {
            continue;
        }
        let (p, w) = committed(carriers, &st.vertices, v)?;
        let c = CarrierIx::Curve(curve_of(arr, e)?);
        let contact = decide(|level: u32| {
            let d = carriers.distance(p, c, level, meter)?;
            Ok(widened(d, w)?)
        })?;
        if let Contact::Touch(_) = contact {
            let t = parameter(arr, e, v, p, meter)?;
            st.paves
                .get_mut(e)
                .ok_or(BuildError::Missing(Entity::Edge(e)))?
                .push(Pave { vertex: v, t });
            st.interferences
                .push(Interference::VertexEdge { vertex: v, edge: e });
        }
    }
    Ok(())
}

/// Edge with edge: where two edges of different operands meet, a pave on each, at a vertex already
/// on either if the meeting touches one, otherwise at a new vertex; undecided regions are held.
fn edge_edge(
    arr: &Arrangement,
    st: &mut Stages,
    pairs: &[(usize, usize)],
    remaining: Bound,
    meter: &mut WorkMeter<'_>,
) -> Result<(), BuildError> {
    let carriers = &arr.carriers;
    for &(a, b) in pairs {
        let (ca, cb) = (curve_of(arr, a)?, curve_of(arr, b)?);
        let (Some(curve_a), Some(curve_b)) = (carriers.curve(ca), carriers.curve(cb)) else {
            return Err(BuildError::Missing(Entity::Edge(a)));
        };
        let met = curves(curve_a, curve_b, meter)?;
        for item in met.items() {
            match item {
                Item::Point { at, xyz, .. } => {
                    let (Some(s), Some(t)) = (at.spans().first(), at.spans().get(1)) else {
                        return Err(IntersectError::Internal.into());
                    };
                    let near: Vec<usize> = st
                        .paves
                        .get(a)
                        .into_iter()
                        .chain(st.paves.get(b))
                        .flatten()
                        .map(|p: &Pave| p.vertex)
                        .collect();
                    let found = touching(carriers, &st.vertices, near.iter().copied(), xyz)?;
                    let (vertex, reused) = if let Some(v) = found {
                        (v, true)
                    } else {
                        st.vertices
                            .push(new_vertex(carriers, [ca, cb], xyz, remaining, meter)?);
                        (st.vertices.len().saturating_sub(1), false)
                    };
                    for (edge, param) in [(a, *s), (b, *t)] {
                        st.paves
                            .get_mut(edge)
                            .ok_or(BuildError::Missing(Entity::Edge(edge)))?
                            .push(Pave { vertex, t: param });
                    }
                    st.interferences.push(Interference::EdgeEdge {
                        first: a,
                        second: b,
                        vertex,
                        reused,
                    });
                }
                // Where two edges share a stretch of one carrier, each one's ends are already on
                // the other as paves; the stretch is found again as a common block.
                Item::Overlap { .. } | Item::Curve { .. } => {}
                Item::Flagged { at, xyz, why } => st.deferred.push(Deferred {
                    edges: (a, b),
                    at: *at,
                    xyz: *xyz,
                    why: *why,
                }),
            }
        }
    }
    Ok(())
}

/// The paves put in order, the edges split, the common blocks found, and every held region
/// checked: it stands only where a common block covers it, or where it touches a vertex both
/// edges pass through, as two edges tangent at a shared corner.
fn finish(
    arr: &Arrangement,
    st: Stages,
    meter: &mut WorkMeter<'_>,
) -> Result<Interfered, BuildError> {
    let mut ordered = Vec::with_capacity(st.paves.len());
    for (e, list) in st.paves.into_iter().enumerate() {
        ordered.push(order(e, list)?);
    }
    let mut splits = Vec::new();
    for (edge, (list, own)) in ordered.iter().zip(&st.ends).enumerate() {
        splits.extend(split(edge, list, own.is_none()));
    }
    let pieces: Vec<PaveBlock> = splits.iter().map(|s: &SplitEdge| s.block).collect();
    let common = common_blocks(arr, &pieces, meter)?;
    for region in &st.deferred {
        let (a, b) = region.edges;
        let shared: Vec<&Pave> = ordered
            .get(a)
            .into_iter()
            .flatten()
            .filter(|p: &&Pave| {
                ordered
                    .get(b)
                    .is_some_and(|list| list.iter().any(|q: &Pave| q.vertex == p.vertex))
            })
            .collect();
        let mut at_corner = false;
        for pave in &shared {
            let (p, w) = committed(&arr.carriers, &st.vertices, pave.vertex)?;
            at_corner |= judge(to_box(p, w, &region.xyz)?) == Verdict::Touch;
        }
        if !at_corner && !covered(region, &splits, &common) {
            return Err(BuildError::Unresolved {
                first: Entity::Edge(a),
                second: Entity::Edge(b),
                why: region.why,
                at: region.xyz,
            });
        }
    }
    Ok(Interfered {
        vertices: st.vertices,
        merged: st.merged,
        paves: ordered,
        ends: st.ends,
        interferences: st.interferences,
        splits,
        common,
        faces: Vec::new(),
        sections: Vec::new(),
    })
}

/// The interferences of `arr`, lowest dimension first: its vertices merged where they touch, each
/// vertex put on the edges of other operands it touches, and each pair of edges of different
/// operands met; every new vertex is held within `remaining` of its carriers.
///
/// # Errors
///
/// [`BuildError::Contact`] for a decision or merge the contact rule cannot take, or a stopped meter;
/// [`BuildError::Unresolved`] for a region an intersection left undecided;
/// [`BuildError::Order`] for paves whose order is not proven; [`BuildError::NotIsolated`] and
/// [`BuildError::Missing`] as they say.
pub fn interfere(
    arr: &Arrangement,
    remaining: Bound,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Interfered, BuildError> {
    let mut st = merge_vertices(arr, remaining, exec, meter)?;
    let found = candidates(arr, &st.vertices, &st.owner, exec, meter)?;
    vertex_edge(arr, &mut st, &found.vertex_edge, meter)?;
    edge_edge(arr, &mut st, &found.edge_edge, remaining, meter)?;
    let pairs = face::candidates(arr, &st, exec, meter)?;
    let mut faces = face::prepare(arr, meter)?;
    face::vertex_face(arr, &mut st, &mut faces, &pairs.vertices, meter)?;
    face::edge_face(arr, &mut st, &mut faces, &pairs.edges, remaining, meter)?;
    let mut out = finish(arr, st, meter)?;
    face::settle(arr, &mut out, faces, &pairs.faces, meter)?;
    Ok(out)
}
