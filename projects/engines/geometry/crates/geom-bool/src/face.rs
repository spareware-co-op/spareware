//! Face information: vertices, edges and faces met with faces, and each face's On, In and section
//! groups (11.4 steps 2 and 4, G243a).
//!
//! What it does: after the vertices and edges have met ([`crate::interfere`]), the faces meet them,
//! lowest dimension first. **Vertex with face:** a vertex of another operand that touches a flat
//! face's plane, by the contact rule, and lies inside the face ([`crate::inside`]) is In it.
//! **Edge with face:** where an edge of another operand crosses a flat face's plane
//! ([`crate::crossing::crossings`]), a point that touches a vertex already on the edge or on the
//! face (its boundary's paves, or a vertex In it) is that vertex, so a meeting through an existing
//! corner ends at the corner; any other point inside the face, or on its boundary, is a new vertex,
//! certified against the edge's curve and the face's plane (and the boundary edges it touches), and
//! a pave on the edge (and on those boundary edges). An edge lying on the plane, by its hull, is
//! kept whole, and once the edges are split, each piece of it whose middle lies inside the face is
//! In. **Face with face:** two flat faces of different operands on distinct planes that are not
//! parallel meet along the line of the two planes, the exact `CurveGeom::Line`; its paves are the
//! vertices the earlier stages put on both faces (on the boundary or In), ordered along the line,
//! and each piece between neighbouring paves whose middle lies inside both faces is a section edge.
//! Each face's information then holds its boundary's split edges and vertices (On), the other
//! operands' vertices and pieces of edges inside it (In), and the section edges across it. A face
//! whose carrier is curved is not placed here: the vertices touching its carrier, and the edges and
//! faces whose boxes meet it, are kept as pending for face splitting, which places them in the
//! face's parameter space (pieces G244 and G245).
//!
//! Guarantees:
//!
//! - **Lowest dimension first, and reused above.** Vertices meet faces before edges do, and every
//!   point where an edge crosses a face is first matched against the vertices already on the edge
//!   and on the face by the contact rule; a section's paves are only vertices already found.
//! - **A section ends at an existing corner.** A corner of one face on the other's plane and inside
//!   it is a vertex of both, so the section line's pave there is that corner, never a second vertex
//!   nearby.
//! - **Flagged regions are accepted only on proof.** A region where an edge meets a face's plane
//!   that the crossing engine could not decide stands only where a vertex the edge and the face
//!   share touches it, or where the region is proven to lie outside the face; an edge proven by its
//!   hull to lie on the plane is a shared stretch and is placed piece by piece. Anything else fails
//!   with [`BuildError::Undecided`].
//! - **Never a guess.** Placing a point is certified ([`crate::inside`]); two section paves whose
//!   order is not proven fail with [`BuildError::SectionOrder`].
//! - **Deterministic.** Pairs come from the broad phase in key order; each list in a face's
//!   information is sorted.
//!
//! Method: chapter 11.4, steps 2 and 4 (`projects/engines/geometry/architecture/11-booleans.md`):
//! the vertex-face, edge-face and face-face interferences and the face information (On, In and
//! section pave blocks) of `OpenCascade`'s General Fuse, studied for method only
//! (`projects/kit/notes/G01-evidence/02-occt-parasolid-acis.md`), decided on carriers by the
//! contact rule. That the lowest-dimension result is taken first is `SMLib`'s, read in solidcore's
//! Rust port (`VertexFaceIntersect`, `EdgeFaceIntersect` and `FaceFaceIntersect` of its topology
//! intersector); its imprinting within a tolerance that grows is not taken.
//!
//! Consulted: solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/c440e5f892452a9654aee8f3030de6095fdb1415/crates/solidcore/src/boolean/topology_intersector/p1.rs>,
//!   for the order of the vertex, edge and face stages; no code copied.
//! SPDX-License-Identifier: Apache-2.0
//!
//! Cost: one broad phase over the vertices, edges and faces; per vertex-face pair one certified
//! distance and at most one placing; per edge-face pair one hull and at most one run of the
//! crossing engine, and a placing per crossing; per face pair the vertices of both faces, sorted
//! once, and two placings per piece of the line; all charged to the meter. No recursion.

use alloc::vec::Vec;

use geom_bvh::{Aabb, Entry};
use geom_intersect::IntersectError;
use geom_intersect::solve::Reason;
use geom_num::grid::COORD_MAX;
use geom_num::interval::Interval;
use geom_num::plane::Plane;
use geom_num::{NumError, P3};
use geom_poly::order::Line as PlaneLine;
use geom_run::{Exec, WorkMeter};
use geom_topo::{Box3, ParamCurve, ParamSurface};

use crate::broad::{candidate_pairs, point_entries};
use crate::carrier::{Bound, CarrierIx, CurveGeom, SortedVec};
use crate::crossing::{Enclosed, Hull, crossings};
use crate::distance::Carriers;
use crate::inside::{FlatFace, Place};
use crate::interfere::{
    Arrangement, BuildError, Entity, Interfered, Interference, Stages, certify, committed,
    coordinates, edge_entry, floor_ceil, nearest_point, parameter, to_box, touching, widened,
};
use crate::pave::{OrderError, Pave, PaveBlock, order};
use crate::rule::{Contact, EPS_FIT, Verdict, decide, judge};

/// What waits for face splitting on a face whose carrier is curved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pending {
    /// Vertices of other operands that touch its carrier, not yet placed in it.
    pub vertices: Vec<usize>,
    /// Edges of other operands whose boxes meet its box.
    pub edges: Vec<usize>,
    /// Faces of other operands whose boxes meet its box.
    pub faces: Vec<usize>,
}

/// A face's information (11.4 step 4).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FaceInfo {
    /// The vertices on its boundary: the paves of its boundary edges.
    pub on_vertices: Vec<usize>,
    /// Its boundary's split edges, by index into [`Interfered::splits`].
    pub on_blocks: Vec<usize>,
    /// Other operands' vertices inside it.
    pub in_vertices: Vec<usize>,
    /// Other operands' split edges lying inside it, by index into [`Interfered::splits`].
    pub in_blocks: Vec<usize>,
    /// The section edges across it, by index into [`Interfered::sections`].
    pub sections: Vec<usize>,
    /// Faces of other operands on the same plane of the table, which face splitting treats as one
    /// carrier.
    pub same_carrier: Vec<usize>,
    /// What waits for face splitting, on a curved face.
    pub pending: Pending,
}

/// A section edge: a piece of the line where the planes of two flat faces meet, inside both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionEdge {
    /// The two faces, lower index first.
    pub faces: [usize; 2],
    /// The line: the meeting of the two faces' planes, exact.
    pub curve: CurveGeom,
    /// The pave it starts at; its parameter is the distance along the line's direction, n₁ × n₂.
    pub from: Pave,
    /// The pave it ends at.
    pub to: Pave,
}

/// The candidate pairs of the face stages, each in key order, of different operands.
pub(crate) struct FacePairs {
    /// Vertex with face.
    pub(crate) vertices: Vec<(usize, usize)>,
    /// Edge with face.
    pub(crate) edges: Vec<(usize, usize)>,
    /// Face with face, lower index first.
    pub(crate) faces: Vec<(usize, usize)>,
}

/// A region where an edge meets a face's plane, undecided, held until the vertices are known.
struct Deferred {
    edge: usize,
    face: usize,
    why: Reason,
    xyz: Box3,
}

/// The working state of the face stages.
pub(crate) struct FaceStage {
    /// Each face placed in, if flat.
    flats: Vec<Option<FlatFace>>,
    /// Each face's information so far.
    info: Vec<FaceInfo>,
    /// Each edge's hull, made when first needed.
    hulls: Vec<Option<Hull>>,
    /// Edges lying on a face's plane, with the face.
    on_plane: Vec<(usize, usize)>,
    /// Undecided regions of edges with faces' planes.
    deferred: Vec<Deferred>,
}

/// A count as a key of the box tree.
fn key(i: usize) -> Result<u64, NumError> {
    u64::try_from(i).map_err(|_| NumError::BudgetExceeded)
}

/// The grid box around the box `lo..hi`, whole steps outward.
fn box_of(lo: [f64; 3], hi: [f64; 3]) -> Result<Box3, NumError> {
    let [lx, ly, lz] = lo;
    let [hx, hy, hz] = hi;
    let min = P3::new(floor_ceil(lx)?.0, floor_ceil(ly)?.0, floor_ceil(lz)?.0)?;
    let max = P3::new(floor_ceil(hx)?.1, floor_ceil(hy)?.1, floor_ceil(hz)?.1)?;
    Box3::new(min, max)
}

/// The box tree entry of a face: its boundary edges' boxes together, and for a curved face its
/// surface's box over its domain as well, or the whole frame where that box is not finite.
fn face_entry(
    arr: &Arrangement,
    f: usize,
    edge_boxes: &[Aabb<3>],
    meter: &mut WorkMeter<'_>,
) -> Result<Entry<3>, BuildError> {
    let face = arr
        .faces
        .get(f)
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    let mut lo = [i64::MAX; 3];
    let mut hi = [i64::MIN; 3];
    let mut grow = |a: [i64; 3], b: [i64; 3]| {
        for k in 0..3 {
            if let (Some(l), Some(h), Some(x), Some(y)) =
                (lo.get_mut(k), hi.get_mut(k), a.get(k), b.get(k))
            {
                *l = (*l).min(*x);
                *h = (*h).max(*y);
            }
        }
    };
    for &e in face.loops.iter().flatten() {
        let b = edge_boxes
            .get(e)
            .ok_or(BuildError::Missing(Entity::Edge(e)))?;
        grow(b.lo(), b.hi());
    }
    if let CarrierIx::Surface(ix) = face.surface {
        let surface = arr
            .carriers
            .surface(ix)
            .ok_or(BuildError::Missing(Entity::Face(f)))?;
        let pad = i64::try_from(EPS_FIT.0).map_err(|_| NumError::BudgetExceeded)?;
        let whole = [COORD_MAX; 3];
        match surface.bound(surface.domain(), meter) {
            Ok(b)
                if b.lo.iter().chain(&b.hi).all(|v| v.is_finite())
                    && box_of(b.lo, b.hi).is_ok() =>
            {
                let g = box_of(b.lo, b.hi)?;
                let (min, max) = (g.min(), g.max());
                let at = |corner: P3, up: bool| -> Result<[i64; 3], NumError> {
                    let shift = |c: i64| {
                        if up {
                            c.checked_add(pad)
                        } else {
                            c.checked_sub(pad)
                        }
                        .ok_or(NumError::BudgetExceeded)
                    };
                    Ok([
                        shift(corner.x.get())?,
                        shift(corner.y.get())?,
                        shift(corner.z.get())?,
                    ])
                };
                grow(at(min, false)?, at(max, true)?);
            }
            _ => grow(whole.map(i64::wrapping_neg), whole),
        }
    }
    if lo.iter().zip(&hi).any(|(l, h)| l > h) {
        return Err(BuildError::Missing(Entity::Face(f)));
    }
    Ok(Entry {
        key: 0,
        bbox: Aabb::new(lo, hi).map_err(crate::rule::ContactError::from)?,
    })
}

/// The pairs the face stages take: every vertex (the merged input vertices and those made where
/// edges meet), edge and face in one box tree, each pair of a face with something of another
/// operand.
///
/// # Errors
///
/// As the broad phase's, and [`BuildError::Missing`] for a face with no boundary.
pub(crate) fn candidates(
    arr: &Arrangement,
    st: &Stages,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<FacePairs, BuildError> {
    let mut out = FacePairs {
        vertices: Vec::new(),
        edges: Vec::new(),
        faces: Vec::new(),
    };
    if arr.faces.is_empty() {
        return Ok(out);
    }
    let nv = st.vertices.len();
    let ne = arr.edges.len();
    let mut entries = point_entries(&arr.carriers, &st.vertices)?;
    let mut edge_boxes = Vec::with_capacity(ne);
    for (e, edge) in arr.edges.iter().enumerate() {
        let entry = edge_entry(
            &arr.carriers,
            (e, edge),
            key(nv.checked_add(e).ok_or(NumError::BudgetExceeded)?)?,
            meter,
        )?;
        edge_boxes.push(entry.bbox);
        entries.push(entry);
    }
    let first_face = nv.checked_add(ne).ok_or(NumError::BudgetExceeded)?;
    for f in 0..arr.faces.len() {
        let mut entry = face_entry(arr, f, &edge_boxes, meter)?;
        entry.key = key(first_face.checked_add(f).ok_or(NumError::BudgetExceeded)?)?;
        entries.push(entry);
    }
    let operand_of_face = |f: usize| arr.faces.get(f).map(|x| x.operand);
    for (a, b) in candidate_pairs(&entries, exec, meter)? {
        let (a, b) = (
            usize::try_from(a).map_err(|_| NumError::BudgetExceeded)?,
            usize::try_from(b).map_err(|_| NumError::BudgetExceeded)?,
        );
        let Some(f) = b.checked_sub(first_face) else {
            continue;
        };
        let face_operand = operand_of_face(f);
        if let Some(g) = a.checked_sub(first_face) {
            if operand_of_face(g) != face_operand {
                out.faces.push((g.min(f), g.max(f)));
            }
        } else if let Some(e) = a.checked_sub(nv) {
            if arr.edges.get(e).map(|x| x.operand) != face_operand {
                out.edges.push((e, f));
            }
        } else {
            // A vertex made where edges meet belongs to no one operand: it is tried with every face.
            let owner = st.owner.get(a).copied();
            if owner != face_operand {
                out.vertices.push((a, f));
            }
        }
    }
    out.vertices.sort_unstable();
    out.edges.sort_unstable();
    out.faces.sort_unstable();
    out.faces.dedup();
    Ok(out)
}

/// Each face made ready: a flat face to place points in, or none for a curved one.
///
/// # Errors
///
/// As [`FlatFace::new`].
pub(crate) fn prepare(
    arr: &Arrangement,
    meter: &mut WorkMeter<'_>,
) -> Result<FaceStage, BuildError> {
    let mut flats = Vec::with_capacity(arr.faces.len());
    for f in 0..arr.faces.len() {
        flats.push(FlatFace::new(arr, f, meter)?);
    }
    Ok(FaceStage {
        flats,
        info: alloc::vec![FaceInfo::default(); arr.faces.len()],
        hulls: alloc::vec![None; arr.edges.len()],
        on_plane: Vec::new(),
        deferred: Vec::new(),
    })
}

/// The vertices on face `f`'s boundary: the paves of its boundary edges, sorted, once each.
fn boundary_vertices(arr: &Arrangement, paves: &[Vec<Pave>], f: usize) -> Vec<usize> {
    let mut out: Vec<usize> = arr
        .faces
        .get(f)
        .into_iter()
        .flat_map(|face| face.loops.iter().flatten())
        .filter_map(|e| paves.get(*e))
        .flatten()
        .map(|p: &Pave| p.vertex)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The face's information and flat form, or a missing face.
fn flat_of(fs: &FaceStage, f: usize) -> Result<Option<&FlatFace>, BuildError> {
    Ok(fs
        .flats
        .get(f)
        .ok_or(BuildError::Missing(Entity::Face(f)))?
        .as_ref())
}

/// The information of face `f`, to change.
fn info_of(fs: &mut FaceStage, f: usize) -> Result<&mut FaceInfo, BuildError> {
    fs.info
        .get_mut(f)
        .ok_or(BuildError::Missing(Entity::Face(f)))
}

/// Vertex with face: a vertex of another operand touching a face's carrier is In a flat face it
/// lies inside, and pending on a curved one.
///
/// # Errors
///
/// As the contact rule's and [`FlatFace::place`]'s.
pub(crate) fn vertex_face(
    arr: &Arrangement,
    st: &mut Stages,
    fs: &mut FaceStage,
    pairs: &[(usize, usize)],
    meter: &mut WorkMeter<'_>,
) -> Result<(), BuildError> {
    let carriers = &arr.carriers;
    for &(v, f) in pairs {
        let face = arr
            .faces
            .get(f)
            .ok_or(BuildError::Missing(Entity::Face(f)))?;
        let (p, w) = committed(carriers, &st.vertices, v)?;
        let contact = decide(|level: u32| {
            let d = carriers.distance(p, face.surface, level, meter)?;
            Ok(widened(d, w)?)
        })?;
        if !matches!(contact, Contact::Touch(_)) {
            continue;
        }
        let Some(flat) = flat_of(fs, f)? else {
            info_of(fs, f)?.pending.vertices.push(v);
            continue;
        };
        if flat.place(carriers, p, w, meter)? == Place::In {
            info_of(fs, f)?.in_vertices.push(v);
            st.interferences
                .push(Interference::VertexFace { vertex: v, face: f });
        }
    }
    Ok(())
}

/// Pushes a pave of `vertex` on `edge` at `t`, unless the vertex is already a pave of it.
fn pave_on(st: &mut Stages, edge: usize, vertex: usize, t: Interval) -> Result<(), BuildError> {
    let list = st
        .paves
        .get_mut(edge)
        .ok_or(BuildError::Missing(Entity::Edge(edge)))?;
    if !list.iter().any(|p: &Pave| p.vertex == vertex) {
        list.push(Pave { vertex, t });
    }
    Ok(())
}

/// The hull of edge `e`, made once.
fn hull_of<'a>(
    arr: &Arrangement,
    hulls: &'a mut [Option<Hull>],
    e: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<&'a Hull, BuildError> {
    let slot = hulls
        .get_mut(e)
        .ok_or(BuildError::Missing(Entity::Edge(e)))?;
    if slot.is_none() {
        let edge = arr
            .edges
            .get(e)
            .ok_or(BuildError::Missing(Entity::Edge(e)))?;
        let curve = arr
            .carriers
            .curve(edge.curve)
            .ok_or(BuildError::Missing(Entity::Edge(e)))?;
        *slot = Some(Hull::of(curve, meter)?);
    }
    slot.as_ref().ok_or(BuildError::Missing(Entity::Edge(e)))
}

/// Where edge `e` crosses the plane of flat face `f`, at `param` within `xyz`: the vertex there, reused
/// or made, put on the edge and on the face (In, or on the boundary edges it touches); nothing when
/// the point is outside the face.
fn edge_face_point(
    arr: &Arrangement,
    st: &mut Stages,
    fs: &mut FaceStage,
    (e, f): (usize, usize),
    (param, xyz): (Interval, &Box3),
    remaining: Bound,
    meter: &mut WorkMeter<'_>,
) -> Result<(), BuildError> {
    let carriers = &arr.carriers;
    let edge = arr
        .edges
        .get(e)
        .ok_or(BuildError::Missing(Entity::Edge(e)))?;
    let face = arr
        .faces
        .get(f)
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    let on_boundary = boundary_vertices(arr, &st.paves, f);
    let inside = info_of(fs, f)?.in_vertices.clone();
    let mut known: Vec<usize> = st
        .paves
        .get(e)
        .into_iter()
        .flatten()
        .map(|p: &Pave| p.vertex)
        .chain(on_boundary.iter().copied())
        .chain(inside.iter().copied())
        .collect();
    known.sort_unstable();
    known.dedup();
    if let Some(v) = touching(carriers, &st.vertices, &known, xyz)? {
        let place = if on_boundary.binary_search(&v).is_ok() {
            Place::On(Vec::new())
        } else if inside.contains(&v) {
            Place::In
        } else {
            let (spot, bound) = committed(carriers, &st.vertices, v)?;
            let flat = flat_of(fs, f)?.ok_or(BuildError::Missing(Entity::Face(f)))?;
            flat.place(carriers, spot, bound, meter)?
        };
        match place {
            Place::Out => return Ok(()),
            Place::In => {
                if !inside.contains(&v) {
                    info_of(fs, f)?.in_vertices.push(v);
                }
            }
            Place::On(sides) => {
                for b in sides {
                    let (spot, _) = committed(carriers, &st.vertices, v)?;
                    let at = parameter(arr, b, v, spot, meter)?;
                    pave_on(st, b, v, at)?;
                }
            }
        }
        pave_on(st, e, v, param)?;
        st.interferences.push(Interference::EdgeFace {
            edge: e,
            face: f,
            vertex: v,
            reused: true,
        });
        return Ok(());
    }
    let ([x0, y0, z0], [x1, y1, z1]) = (coordinates(xyz.min()), coordinates(xyz.max()));
    let at = nearest_point([
        x0 / 2.0 + x1 / 2.0,
        y0 / 2.0 + y1 / 2.0,
        z0 / 2.0 + z1 / 2.0,
    ])?;
    let on = SortedVec::from_items([CarrierIx::Curve(edge.curve), face.surface]);
    let point = certify(carriers, at, on.clone(), remaining, meter)?;
    let (_, within) = carriers.commit(&point)?;
    let flat = flat_of(fs, f)?.ok_or(BuildError::Missing(Entity::Face(f)))?;
    let (point, sides) = match flat.place(carriers, at, within, meter)? {
        Place::Out => return Ok(()),
        Place::In => (point, Vec::new()),
        Place::On(sides) => {
            let mut all = on.clone();
            for &b in &sides {
                let curve = arr
                    .edges
                    .get(b)
                    .ok_or(BuildError::Missing(Entity::Edge(b)))?
                    .curve;
                all = all.union(&SortedVec::from_items([CarrierIx::Curve(curve)]));
            }
            (certify(carriers, at, all, remaining, meter)?, sides)
        }
    };
    st.vertices.push(point);
    let v = st.vertices.len().saturating_sub(1);
    pave_on(st, e, v, param)?;
    if sides.is_empty() {
        info_of(fs, f)?.in_vertices.push(v);
    }
    for b in sides {
        let at = parameter(arr, b, v, at, meter)?;
        pave_on(st, b, v, at)?;
    }
    st.interferences.push(Interference::EdgeFace {
        edge: e,
        face: f,
        vertex: v,
        reused: false,
    });
    Ok(())
}

/// Edge with face: where each edge of another operand crosses a flat face's plane, a vertex on the
/// edge and on the face; an edge lying on the plane is held to be placed piece by piece; undecided
/// regions are held; on a curved face, the edge is pending.
///
/// # Errors
///
/// As the crossing engine's, the contact rule's and [`FlatFace::place`]'s;
/// [`crate::rule::CoincidenceChain`] for a new vertex that cannot be held within `remaining`.
pub(crate) fn edge_face(
    arr: &Arrangement,
    st: &mut Stages,
    fs: &mut FaceStage,
    pairs: &[(usize, usize)],
    remaining: Bound,
    meter: &mut WorkMeter<'_>,
) -> Result<(), BuildError> {
    for &(e, f) in pairs {
        let Some(flat) = flat_of(fs, f)? else {
            info_of(fs, f)?.pending.edges.push(e);
            continue;
        };
        let plane = *flat.plane();
        let enclosed = Enclosed::of(&plane)?;
        match judge(hull_of(arr, &mut fs.hulls, e, meter)?.spread(&enclosed)?) {
            Verdict::Decided(_) => continue,
            Verdict::Touch => {
                fs.on_plane.push((e, f));
                continue;
            }
            Verdict::Near => {}
        }
        let edge = arr
            .edges
            .get(e)
            .ok_or(BuildError::Missing(Entity::Edge(e)))?;
        let curve = arr
            .carriers
            .curve(edge.curve)
            .ok_or(BuildError::Missing(Entity::Edge(e)))?;
        let found = crossings(curve, &plane, meter)?;
        for zero in &found.zeros {
            let [t] = zero.enclosure;
            let b = curve
                .bound((t.lo(), t.hi()), meter)
                .map_err(IntersectError::from)?;
            let xyz = box_of(b.lo, b.hi)?;
            edge_face_point(arr, st, fs, (e, f), (t, &xyz), remaining, meter)?;
        }
        for flag in &found.flagged {
            let [t] = flag.region;
            let b = curve
                .bound((t.lo(), t.hi()), meter)
                .map_err(IntersectError::from)?;
            fs.deferred.push(Deferred {
                edge: e,
                face: f,
                why: flag.why,
                xyz: box_of(b.lo, b.hi)?,
            });
        }
    }
    Ok(())
}

/// A parameter strictly inside a block of `curve`, between its paves' enclosures, or `None` when
/// the enclosures leave no room.
fn middle_of(curve: &impl ParamCurve<3>, block: &PaveBlock) -> Option<f64> {
    let (a, b) = curve.domain();
    let between = |lo: f64, hi: f64| (lo < hi).then_some(lo / 2.0 + hi / 2.0);
    match (block.from, block.to) {
        (Some(from), Some(to)) if !block.wraps => between(from.t.hi(), to.t.lo()),
        (Some(from), Some(to)) => between(from.t.hi(), b).or_else(|| between(a, to.t.lo())),
        _ => between(a, b),
    }
}

/// The grid point nearest the curve at `t`, and a bound on how far it is from it.
fn near_point(
    curve: &impl ParamCurve<3>,
    t: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<(P3, Bound), BuildError> {
    let b = curve.bound((t, t), meter).map_err(IntersectError::from)?;
    let mut centre = [0.0; 3];
    let mut reach = 0.0_f64;
    for ((c, lo), hi) in centre.iter_mut().zip(&b.lo).zip(&b.hi) {
        *c = lo / 2.0 + hi / 2.0;
        let w = hi - lo;
        if w > reach {
            reach = w;
        }
    }
    let at = nearest_point(centre)?;
    // The rounding to the grid moves a point at most half a step in each coordinate, so less than
    // a step in all; the box adds its widest side.
    let w = Bound::of(reach + 1.0).ok_or(NumError::BudgetExceeded)?;
    Ok((at, w))
}

/// Whether a vertex the edge and the face share touches a held region.
fn shared_touch(
    carriers: &Carriers,
    out: &Interfered,
    info: &FaceInfo,
    on_boundary: &[usize],
    region: &Deferred,
) -> Result<bool, BuildError> {
    for pave in out.paves.get(region.edge).into_iter().flatten() {
        let v = pave.vertex;
        if on_boundary.binary_search(&v).is_err() && !info.in_vertices.contains(&v) {
            continue;
        }
        let (p, w) = committed(carriers, &out.vertices, v)?;
        if judge(to_box(p, w, &region.xyz)?) == Verdict::Touch {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The paves of the line where the planes `pa` of face `f1` and `pb` of face `f2` meet: the
/// vertices the earlier stages put on both faces (on the boundary or In), each with the enclosure
/// of its distance along the line's direction n₁ × n₂, widened by its bound.
fn section_paves(
    arr: &Arrangement,
    out: &Interfered,
    fs: &FaceStage,
    (f1, f2): (usize, usize),
    (pa, pb): (&Plane, &Plane),
) -> Result<Vec<Pave>, BuildError> {
    let (ea, eb) = (Enclosed::of(pa)?, Enclosed::of(pb)?);
    let ([a0, a1, a2], [b0, b1, b2]) = (ea.n, eb.n);
    let direction = [
        a1.mul(b2)?.sub(a2.mul(b1)?)?,
        a2.mul(b0)?.sub(a0.mul(b2)?)?,
        a0.mul(b1)?.sub(a1.mul(b0)?)?,
    ];
    let length = Interval::norm(&direction)?;
    let on = |f: usize| -> Vec<usize> {
        let mut list = boundary_vertices(arr, &out.paves, f);
        if let Some(info) = fs.info.get(f) {
            list.extend(info.in_vertices.iter().copied());
        }
        list.sort_unstable();
        list.dedup();
        list
    };
    let (first, second) = (on(f1), on(f2));
    let mut paves = Vec::new();
    for v in first.iter().filter(|v| second.binary_search(v).is_ok()) {
        let (spot, bound) = committed(&arr.carriers, &out.vertices, *v)?;
        let [cx, cy, cz] = coordinates(spot);
        let [d0, d1, d2] = direction;
        let along = Interval::point(cx)?
            .mul(d0)?
            .add(Interval::point(cy)?.mul(d1)?)?
            .add(Interval::point(cz)?.mul(d2)?)?
            .div(length)?;
        #[allow(
            clippy::cast_precision_loss,
            reason = "bounds are counts of steps far below 2^53"
        )]
        let slack = bound.steps() as f64;
        paves.push(Pave {
            vertex: *v,
            t: Interval::new(
                (along.lo() - slack).next_down(),
                (along.hi() + slack).next_up(),
            )?,
        });
    }
    Ok(paves)
}

/// Whether a held region is proven outside its face: the grid point nearest its box's centre, with
/// a bound reaching every corner of the box, placed outside the face. Placing it Out proves the
/// ball round it apart from every boundary edge, so the whole box lies on the outer side.
fn outside(
    carriers: &Carriers,
    fs: &FaceStage,
    region: &Deferred,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, BuildError> {
    let Some(flat) = flat_of(fs, region.face)? else {
        return Ok(false);
    };
    let ([x0, y0, z0], [x1, y1, z1]) =
        (coordinates(region.xyz.min()), coordinates(region.xyz.max()));
    let centre = nearest_point([
        x0 / 2.0 + x1 / 2.0,
        y0 / 2.0 + y1 / 2.0,
        z0 / 2.0 + z1 / 2.0,
    ])?;
    // Half the box's diagonal is at most half the sum of its sides, and the rounding to the grid
    // adds less than a step.
    let reach = ((x1 - x0) + (y1 - y0) + (z1 - z0)) / 2.0 + 1.0;
    let bound = Bound::of(reach).ok_or(NumError::BudgetExceeded)?;
    match flat.place(carriers, centre, bound, meter) {
        Ok(Place::Out) => Ok(true),
        Ok(_) | Err(BuildError::Unclassified { .. }) => Ok(false),
        Err(e) => Err(e),
    }
}

/// The section edges where the planes of flat faces `f1` and `f2` meet, kept inside both.
fn section(
    arr: &Arrangement,
    out: &mut Interfered,
    fs: &mut FaceStage,
    (f1, f2): (usize, usize),
    meter: &mut WorkMeter<'_>,
) -> Result<(), BuildError> {
    if flat_of(fs, f1)?.is_none() || flat_of(fs, f2)?.is_none() {
        // A curved face on either side: the pair waits for face splitting.
        info_of(fs, f1)?.pending.faces.push(f2);
        info_of(fs, f2)?.pending.faces.push(f1);
        return Ok(());
    }
    let (s1, s2) = (
        arr.faces.get(f1).map(|x| x.surface),
        arr.faces.get(f2).map(|x| x.surface),
    );
    let (Some(CarrierIx::Plane(p1)), Some(CarrierIx::Plane(p2))) = (s1, s2) else {
        return Err(BuildError::Missing(Entity::Face(f1)));
    };
    if p1 == p2 {
        info_of(fs, f1)?.same_carrier.push(f2);
        info_of(fs, f2)?.same_carrier.push(f1);
        return Ok(());
    }
    let (Some(first_flat), Some(second_flat)) = (flat_of(fs, f1)?, flat_of(fs, f2)?) else {
        return Err(BuildError::Missing(Entity::Face(f1)));
    };
    let (pa, pb): (Plane, Plane) = (*first_flat.plane(), *second_flat.plane());
    match PlaneLine::new(pa, pb) {
        Err(NumError::Degenerate) => return Ok(()),
        Err(e) => return Err(e.into()),
        Ok(_) => {}
    }
    let paves = section_paves(arr, out, fs, (f1, f2), (&pa, &pb))?;
    let ordered = order(f1, paves).map_err(|e| match e {
        OrderError::Undecided(p) => BuildError::SectionOrder {
            faces: [f1, f2],
            first: p.first,
            second: p.second,
        },
        OrderError::Num(n) => n.into(),
    })?;
    let mut kept = Vec::new();
    for (from, to) in ordered.iter().zip(ordered.iter().skip(1)) {
        let (pf, _) = committed(&arr.carriers, &out.vertices, from.vertex)?;
        let (pt, _) = committed(&arr.carriers, &out.vertices, to.vertex)?;
        let ([x0, y0, z0], [x1, y1, z1]) = (coordinates(pf), coordinates(pt));
        let middle = nearest_point([
            x0 / 2.0 + x1 / 2.0,
            y0 / 2.0 + y1 / 2.0,
            z0 / 2.0 + z1 / 2.0,
        ])?;
        let carriers = &arr.carriers;
        let here = |flat: &FlatFace, meter: &mut WorkMeter<'_>| {
            flat.place(carriers, middle, Bound::ZERO, meter)
        };
        if here(first_flat, meter)? == Place::In && here(second_flat, meter)? == Place::In {
            kept.push((*from, *to));
        }
    }
    for (from, to) in kept {
        let index = out.sections.len();
        out.sections.push(SectionEdge {
            faces: [f1, f2],
            curve: CurveGeom::Line([p1, p2]),
            from,
            to,
        });
        out.interferences.push(Interference::FaceFace {
            first: f1,
            second: f2,
            section: index,
        });
        info_of(fs, f1)?.sections.push(index);
        info_of(fs, f2)?.sections.push(index);
    }
    Ok(())
}

/// The face stages finished once the edges are split: held regions checked, edges lying on a
/// face's plane placed piece by piece, the sections made, and each face's On group filled.
///
/// # Errors
///
/// [`BuildError::Undecided`] for a held region no shared vertex covers; as [`FlatFace::place`]'s;
/// [`BuildError::SectionOrder`] for section paves whose order is not proven.
pub(crate) fn settle(
    arr: &Arrangement,
    out: &mut Interfered,
    mut fs: FaceStage,
    face_pairs: &[(usize, usize)],
    meter: &mut WorkMeter<'_>,
) -> Result<(), BuildError> {
    let carriers = &arr.carriers;
    for region in &fs.deferred {
        let on_boundary = boundary_vertices(arr, &out.paves, region.face);
        let info = fs
            .info
            .get(region.face)
            .ok_or(BuildError::Missing(Entity::Face(region.face)))?;
        if shared_touch(carriers, out, info, &on_boundary, region)?
            || outside(carriers, &fs, region, meter)?
        {
            continue;
        }
        return Err(BuildError::Undecided {
            edge: region.edge,
            face: region.face,
            why: region.why,
            at: region.xyz,
        });
    }
    // The split edges of each edge, by index, in order.
    let mut by_source: Vec<Vec<usize>> = alloc::vec![Vec::new(); arr.edges.len()];
    for (k, piece) in out.splits.iter().enumerate() {
        by_source
            .get_mut(piece.source)
            .ok_or(BuildError::Missing(Entity::Edge(piece.source)))?
            .push(k);
    }
    let on_plane = core::mem::take(&mut fs.on_plane);
    for (e, f) in on_plane {
        let edge = arr
            .edges
            .get(e)
            .ok_or(BuildError::Missing(Entity::Edge(e)))?;
        let curve = carriers
            .curve(edge.curve)
            .ok_or(BuildError::Missing(Entity::Edge(e)))?;
        for &k in by_source.get(e).into_iter().flatten() {
            let Some(piece) = out.splits.get(k) else {
                continue;
            };
            let Some(t) = middle_of(curve, &piece.block) else {
                continue;
            };
            let (p, w) = near_point(curve, t, meter)?;
            let flat = flat_of(&fs, f)?.ok_or(BuildError::Missing(Entity::Face(f)))?;
            if flat.place(carriers, p, w, meter)? == Place::In {
                info_of(&mut fs, f)?.in_blocks.push(k);
            }
        }
    }
    for &pair in face_pairs {
        section(arr, out, &mut fs, pair, meter)?;
    }
    for (f, face) in arr.faces.iter().enumerate() {
        let blocks: Vec<usize> = face
            .loops
            .iter()
            .flatten()
            .filter_map(|e| by_source.get(*e))
            .flatten()
            .copied()
            .collect();
        let vertices = boundary_vertices(arr, &out.paves, f);
        let info = info_of(&mut fs, f)?;
        info.on_blocks = blocks;
        info.on_vertices = vertices;
        for list in [
            &mut info.on_blocks,
            &mut info.in_vertices,
            &mut info.in_blocks,
            &mut info.sections,
            &mut info.same_carrier,
            &mut info.pending.vertices,
            &mut info.pending.edges,
            &mut info.pending.faces,
        ] {
            list.sort_unstable();
            list.dedup();
        }
    }
    out.faces = fs.info;
    Ok(())
}
