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
//! operands' vertices and pieces of edges inside it (In), and the section edges across it.
//! **A face on a cylinder, a cone, a sphere or a torus** (G244; a torus's in one half of it, its
//! axis along a coordinate) is placed in as a flat one is, in its own picture
//! (`crate::picture`): a vertex of another operand touching its carrier and inside it is In, and
//! so is each piece of an edge of a face of another operand on the same carrier (two faces on one
//! carrier, as two on one plane, found exactly for cylinders given by whole numbers and by their
//! stored data for cones, spheres and tori). Any other edge meets the carrier where the curve and
//! surface
//! intersection finds (`geom_intersect::curve_surface`, G164): each point inside the face, or on
//! its boundary, is a vertex as on a flat face, and an edge found to lie on the carrier is placed
//! piece by piece. **A curved face with a face on another carrier** (a plane of the table, or
//! another carrier the closed forms take) meets it where `geom_intersect::quadrics::natural` (G165
//! to G167) finds the two carriers cross, or two quadrics touch exactly, in closed form: each curve
//! it finds is taken whole (a circle or an ellipse round its full turn) into
//! [`crate::interfere::Interfered::curves`], its paves are the vertices both faces hold, each on
//! the curve proven nearest to it, at the parameter of its nearest point there, and a vertex at a
//! cone's apex, made where the apex lies inside both faces, on every curve that ends there; each
//! piece between neighbouring paves (round the seam of a closed curve; the whole curve when none is
//! on it) whose middle lies inside both faces is a section edge. What the closed forms do not
//! settle waits as pending: a pair in general position (whose curve is a procedural one), carriers
//! that touch at a point or along a curve not proven exact, a curve ending at a flagged region, a
//! cone's apex on a face's boundary away from its vertices, and a plane whose table form no
//! surface holds exactly. A face of another operand on any other carrier whose box meets a curved
//! face, and everything about a face on any other curved carrier, is pending too; a face with
//! something pending is not split (pieces G244 and G245).
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
//! crossing engine (with a cylinder's or cone's face, one curve and surface intersection), and a
//! placing per crossing; per face pair the vertices of both faces, sorted
//! once, and two placings per piece of the line; all charged to the meter. No recursion.

use alloc::vec::Vec;

use geom_bvh::{Aabb, Entry};
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::project::{Projection, project_curve};
use geom_intersect::quadrics::natural;
use geom_intersect::solve::Reason;
use geom_intersect::{CurveContact, CurveEnd, IntersectError, Item, Unresolved};
use geom_num::grid::COORD_MAX;
use geom_num::interval::Interval;
use geom_num::plane::Plane;
use geom_num::quadric::Cylinder;
use geom_num::{NumError, P3};
use geom_poly::order::Line as PlaneLine;
use geom_run::{Exec, WorkMeter};
use geom_topo::{
    Box3, Circle, CurveKind, Ellipse, FlatSurface, ParamCurve, ParamSurface, SurfaceKind,
};

use crate::broad::{candidate_pairs, point_entries};
use crate::carrier::{Bound, CarrierIx, CurveGeom, SortedVec};
use crate::crossing::{Enclosed, Hull, crossings};
use crate::decide::cylinders_coincide;
use crate::distance::{Carriers, FIRST_FLOOR};
use crate::inside::{FlatFace, Place, RoundFace};
use crate::interfere::{
    Arrangement, BuildError, Entity, Interfered, Interference, Stages, certify, committed,
    coordinates, edge_entry, floor_ceil, nearest_point, parameter, to_box, touching, widened,
};
use crate::pave::{OrderError, Pave, PaveBlock, blocks, order};
use crate::rule::{
    Contact, ContactError, EPS_FIT, NearContact, REFINEMENTS, Verdict, decide, greater, judge,
    lesser,
};

/// What waits for face splitting on a face whose carrier is curved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pending {
    /// Vertices of other operands that touch its carrier, not yet placed in it: none on a
    /// cylinder or a cone, where they are placed.
    pub vertices: Vec<usize>,
    /// Edges of other operands whose boxes meet its box: none on a cylinder or a cone, where they
    /// are met.
    pub edges: Vec<usize>,
    /// Faces of other operands whose boxes meet its box, but for those on its carrier.
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
    /// Faces of other operands on the same carrier, which face splitting treats as one: the same
    /// plane of the table, or a curved carrier that coincides with this face's.
    pub same_carrier: Vec<usize>,
    /// What waits for face splitting, on a curved face.
    pub pending: Pending,
}

/// A section edge: a piece of the line where the planes of two flat faces meet, or of a curve
/// where the carriers of two faces meet, inside both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionEdge {
    /// The two faces, lower index first.
    pub faces: [usize; 2],
    /// The line, the meeting of the two faces' planes, exact; or the curve, by its index in
    /// [`crate::interfere::Interfered::curves`].
    pub curve: CurveGeom,
    /// The pave it starts at: on a line, its parameter is the distance along the line's direction,
    /// n₁ × n₂; on a curve, the curve's own. `None` only for a closed curve with no pave.
    pub from: Option<Pave>,
    /// The pave it ends at; `None` only for a closed curve with no pave.
    pub to: Option<Pave>,
    /// Whether it runs round a closed curve's seam, from its last pave to its first.
    pub wraps: bool,
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
    /// Each face placed in, if on a cylinder or a cone.
    rounds: Vec<Option<RoundFace>>,
    /// Each face's information so far.
    info: Vec<FaceInfo>,
    /// Each edge's hull, made when first needed.
    hulls: Vec<Option<Hull>>,
    /// Edges lying on a face's plane, with the face.
    on_plane: Vec<(usize, usize)>,
    /// Edges lying on a curved face's carrier, with the face: the edges of a face of another
    /// operand on the same carrier.
    on_carrier: Vec<(usize, usize)>,
    /// Undecided regions of edges with faces' planes.
    deferred: Vec<Deferred>,
    /// Each face's boundary vertices, sorted, made when first needed and dropped when a pave is
    /// put on one of its edges.
    boundary: Vec<Option<Vec<usize>>>,
    /// The faces each edge bounds.
    faces_of_edge: Vec<Vec<usize>>,
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
        for ((low, high), (x, y)) in lo.iter_mut().zip(hi.iter_mut()).zip(a.into_iter().zip(b)) {
            // Named types, so the recursion check can read the comparisons.
            let (low, high, x, y): (&mut i64, &mut i64, i64, i64) = (low, high, x, y);
            if x < *low {
                *low = x;
            }
            if y > *high {
                *high = y;
            }
        }
    };
    for e in face.loops.iter().flatten().map(|u| u.edge) {
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

/// Each face made ready: a flat face, or one on a cylinder or a cone, to place points in, or
/// none for any other.
///
/// # Errors
///
/// As [`FlatFace::new`] and [`RoundFace::new`].
pub(crate) fn prepare(
    arr: &Arrangement,
    meter: &mut WorkMeter<'_>,
) -> Result<FaceStage, BuildError> {
    let mut flats = Vec::with_capacity(arr.faces.len());
    let mut rounds = Vec::with_capacity(arr.faces.len());
    for f in 0..arr.faces.len() {
        let flat = FlatFace::new(arr, f, meter)?;
        rounds.push(match flat {
            Some(_) => None,
            None => RoundFace::new(arr, f, meter)?,
        });
        flats.push(flat);
    }
    let mut faces_of_edge = alloc::vec![Vec::new(); arr.edges.len()];
    for (f, face) in arr.faces.iter().enumerate() {
        for e in face.loops.iter().flatten().map(|u| u.edge) {
            faces_of_edge
                .get_mut(e)
                .ok_or(BuildError::Missing(Entity::Edge(e)))?
                .push(f);
        }
    }
    Ok(FaceStage {
        flats,
        rounds,
        info: alloc::vec![FaceInfo::default(); arr.faces.len()],
        hulls: alloc::vec![None; arr.edges.len()],
        on_plane: Vec::new(),
        on_carrier: Vec::new(),
        deferred: Vec::new(),
        boundary: alloc::vec![None; arr.faces.len()],
        faces_of_edge,
    })
}

/// The vertices on face `f`'s boundary: the paves of its boundary edges, sorted, once each.
fn boundary_vertices(arr: &Arrangement, paves: &[Vec<Pave>], f: usize) -> Vec<usize> {
    let mut out: Vec<usize> = arr
        .faces
        .get(f)
        .into_iter()
        .flat_map(|face| face.loops.iter().flatten())
        .filter_map(|u| paves.get(u.edge))
        .flatten()
        .map(|p: &Pave| p.vertex)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Face `f`'s boundary vertices, sorted, from the stage's store.
fn boundary_of(
    fs: &mut FaceStage,
    arr: &Arrangement,
    paves: &[Vec<Pave>],
    f: usize,
) -> Result<Vec<usize>, BuildError> {
    let slot = fs
        .boundary
        .get_mut(f)
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    Ok(slot
        .get_or_insert_with(|| boundary_vertices(arr, paves, f))
        .clone())
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

/// Where the grid point `p`, within `w` of where its vertex is, lies in face `f`, if `f` is a
/// face points are placed in: a flat one, or one on a cylinder or a cone.
fn place_in(
    fs: &FaceStage,
    carriers: &Carriers,
    f: usize,
    (p, w): (P3, Bound),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Place>, BuildError> {
    if let Some(flat) = flat_of(fs, f)? {
        return flat.place(carriers, p, w, meter).map(Some);
    }
    match fs.rounds.get(f).and_then(Option::as_ref) {
        Some(round) => round.place(carriers, p, w, meter).map(Some),
        None => Ok(None),
    }
}

/// A cylinder given by whole numbers, as the exact decisions take it: its centre a grid point,
/// its axis a whole vector and its radius a whole number of steps. `None` for any other.
fn whole_cylinder(centre: [f64; 3], normal: [f64; 3], radius: f64) -> Option<Cylinder> {
    let grid = |v: [f64; 3]| nearest_point(v).ok().filter(|p| coordinates(*p) == v);
    let base = grid(centre)?;
    let ([c0, c1, c2], [n0, n1, n2]) = (centre, normal);
    let along = grid([c0 + n0, c1 + n1, c2 + n2])?;
    // The sum is the axis's other point only where it lost nothing.
    let [a0, a1, a2] = coordinates(along);
    if [a0 - c0, a1 - c1, a2 - c2] != normal {
        return None;
    }
    let steps = Bound::of(radius)?;
    #[allow(
        clippy::cast_precision_loss,
        reason = "a count of steps a double gave, compared with that double"
    )]
    let same = steps.steps() as f64 == radius;
    same.then_some(Cylinder {
        base,
        along,
        radius: steps.0,
    })
}

/// Whether faces `f1` and `f2` lie on one curved carrier: the same surface of the pool; two
/// cylinders given by whole numbers that coincide, decided exactly
/// ([`crate::decide::cylinders_coincide`]); two cones with the same stored centre, frame,
/// radius, slope and axial factor; two spheres with the same stored centre and radius; or two
/// tori with the same stored centre and radii about one axis. Any other pair is not known to,
/// and waits.
fn coincide(arr: &Arrangement, f1: usize, f2: usize) -> Result<bool, BuildError> {
    let surface_of = |f: usize| arr.faces.get(f).map(|x| x.surface);
    let (Some(CarrierIx::Surface(a)), Some(CarrierIx::Surface(b))) =
        (surface_of(f1), surface_of(f2))
    else {
        return Ok(false);
    };
    if a == b {
        return Ok(true);
    }
    let (Some(first), Some(second)) = (arr.carriers.surface(a), arr.carriers.surface(b)) else {
        return Err(BuildError::Missing(Entity::Face(f1)));
    };
    match (first, second) {
        (SurfaceKind::Cylinder(c1), SurfaceKind::Cylinder(c2)) => {
            let exact = (
                whole_cylinder(c1.center(), c1.frame().normal(), c1.radius()),
                whole_cylinder(c2.center(), c2.frame().normal(), c2.radius()),
            );
            match exact {
                (Some(x), Some(y)) => Ok(cylinders_coincide(&x, &y)?),
                _ => Ok(false),
            }
        }
        (SurfaceKind::Cone(c1), SurfaceKind::Cone(c2)) => Ok(c1.center() == c2.center()
            && c1.frame() == c2.frame()
            && c1.radius_and_slope() == c2.radius_and_slope()
            && c1.axial_sign() == c2.axial_sign()),
        // A sphere is its centre and radius, whatever frame its parameters are taken on.
        (SurfaceKind::Sphere(s1), SurfaceKind::Sphere(s2)) => {
            Ok(s1.center() == s2.center() && s1.radius() == s2.radius())
        }
        // A torus is its centre, its axis's line and its radii, whichever way the axis points.
        (SurfaceKind::Torus(t1), SurfaceKind::Torus(t2)) => {
            let (n1, n2) = (t1.frame().normal(), t2.frame().normal());
            Ok(t1.center() == t2.center()
                && t1.radii() == t2.radii()
                && (n1 == n2 || n1 == t2.frame().reversed().normal()))
        }
        _ => Ok(false),
    }
}

/// Vertex with face: a vertex of another operand touching a face's carrier is In the face it lies
/// inside, a flat one or one on a cylinder or a cone, and pending on any other curved face.
///
/// # Errors
///
/// As the contact rule's, [`FlatFace::place`]'s and [`RoundFace::place`]'s.
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
        // A vertex already on the face's boundary, by the earlier stages, is On it.
        let on_boundary = fs
            .boundary
            .get_mut(f)
            .ok_or(BuildError::Missing(Entity::Face(f)))?
            .get_or_insert_with(|| boundary_vertices(arr, &st.paves, f));
        if on_boundary.binary_search(&v).is_ok() {
            continue;
        }
        let (p, w) = committed(carriers, &st.vertices, v)?;
        // A cylinder's or cone's distance is read in closed form, from the whole carrier.
        let round = fs.rounds.get(f).and_then(Option::as_ref);
        let contact = decide(|level: u32| {
            let d = match round {
                Some(round) => round.gap(p)?,
                None => carriers.distance(p, face.surface, level, meter)?,
            };
            Ok(widened(d, w)?)
        })?;
        if !matches!(contact, Contact::Touch(_)) {
            continue;
        }
        let Some(place) = place_in(fs, carriers, f, (p, w), meter)? else {
            info_of(fs, f)?.pending.vertices.push(v);
            continue;
        };
        if place == Place::In {
            info_of(fs, f)?.in_vertices.push(v);
            st.interferences
                .push(Interference::VertexFace { vertex: v, face: f });
        }
    }
    Ok(())
}

/// Pushes a pave of `vertex` on `edge` at `t`, unless the vertex is already a pave of it, and
/// drops the stored boundary vertices of the faces the edge bounds.
fn pave_on(
    st: &mut Stages,
    fs: &mut FaceStage,
    edge: usize,
    vertex: usize,
    t: Interval,
) -> Result<(), BuildError> {
    let list = st
        .paves
        .get_mut(edge)
        .ok_or(BuildError::Missing(Entity::Edge(edge)))?;
    if !list.iter().any(|p: &Pave| p.vertex == vertex) {
        list.push(Pave { vertex, t });
        for &f in fs.faces_of_edge.get(edge).into_iter().flatten() {
            if let Some(slot) = fs.boundary.get_mut(f) {
                *slot = None;
            }
        }
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

/// The vertex `v` already there, where edge `e` crosses the plane of flat face `f` at `param`: put
/// on the edge and on the face as it lies there, In or on the boundary; nothing when it lies
/// outside the face.
fn reuse_point(
    arr: &Arrangement,
    st: &mut Stages,
    fs: &mut FaceStage,
    (e, f, v): (usize, usize, usize),
    param: Interval,
    (on_boundary, inside): (&[usize], &[usize]),
    meter: &mut WorkMeter<'_>,
) -> Result<(), BuildError> {
    let carriers = &arr.carriers;
    let place = if on_boundary.binary_search(&v).is_ok() {
        Place::On(Vec::new())
    } else if inside.contains(&v) {
        Place::In
    } else {
        let (spot, bound) = committed(carriers, &st.vertices, v)?;
        place_in(fs, carriers, f, (spot, bound), meter)?
            .ok_or(BuildError::Missing(Entity::Face(f)))?
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
                pave_on(st, fs, b, v, at)?;
            }
        }
    }
    pave_on(st, fs, e, v, param)?;
    st.interferences.push(Interference::EdgeFace {
        edge: e,
        face: f,
        vertex: v,
        reused: true,
    });
    Ok(())
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
    let on_boundary = boundary_of(fs, arr, &st.paves, f)?;
    let inside = info_of(fs, f)?.in_vertices.clone();
    let known = st
        .paves
        .get(e)
        .into_iter()
        .flatten()
        .map(|p: &Pave| p.vertex)
        .chain(on_boundary.iter().copied())
        .chain(inside.iter().copied());
    if let Some(v) = touching(carriers, &st.vertices, known, xyz)? {
        return reuse_point(
            arr,
            st,
            fs,
            (e, f, v),
            param,
            (&on_boundary, &inside),
            meter,
        );
    }
    let (at, _) = centre_of(xyz)?;
    let on = SortedVec::from_items([CarrierIx::Curve(edge.curve), face.surface]);
    let point = certify(carriers, at, on.clone(), remaining, meter)?;
    let (_, within) = carriers.commit(&point)?;
    let place = place_in(fs, carriers, f, (at, within), meter)?
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    let (point, sides) = match place {
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
    pave_on(st, fs, e, v, param)?;
    if sides.is_empty() {
        info_of(fs, f)?.in_vertices.push(v);
    }
    for b in sides {
        let at = parameter(arr, b, v, at, meter)?;
        pave_on(st, fs, b, v, at)?;
    }
    st.interferences.push(Interference::EdgeFace {
        edge: e,
        face: f,
        vertex: v,
        reused: false,
    });
    Ok(())
}

/// Why a region where a curve meets a surface was left undecided, as the face stages report it.
fn reason_of(why: Unresolved) -> Reason {
    match why {
        Unresolved::TangentBand { .. }
        | Unresolved::HigherOrderContact
        | Unresolved::NearCoincident { .. } => Reason::Singular,
        Unresolved::NonSmooth { .. } => Reason::NotSmooth,
        Unresolved::NearMiss { .. }
        | Unresolved::BoundExceeded { .. }
        | Unresolved::KnownCurveUnproven
        | Unresolved::UndecidedAtFloor { .. } => Reason::Unproven,
    }
}

/// Edge `e` with face `f` on a cylinder or a cone, the edge bounding no face on its carrier: where
/// the edge's curve meets the carrier ([`geom_intersect::curve_surface::curve_surface`], G164),
/// each point a vertex on the edge and on the face as on a flat one; an edge lying on the carrier
/// is held to be placed piece by piece; undecided regions are held.
fn edge_round(
    arr: &Arrangement,
    st: &mut Stages,
    fs: &mut FaceStage,
    (e, f): (usize, usize),
    remaining: Bound,
    meter: &mut WorkMeter<'_>,
) -> Result<(), BuildError> {
    let edge = arr
        .edges
        .get(e)
        .ok_or(BuildError::Missing(Entity::Edge(e)))?;
    let curve = arr
        .carriers
        .curve(edge.curve)
        .ok_or(BuildError::Missing(Entity::Edge(e)))?;
    let face = arr
        .faces
        .get(f)
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    let CarrierIx::Surface(ix) = face.surface else {
        return Err(BuildError::Missing(Entity::Face(f)));
    };
    let surface = arr
        .carriers
        .surface(ix)
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    let found = curve_surface(curve, surface, meter)?;
    let mut lies = false;
    for item in found.items() {
        match item {
            Item::Point { at, xyz, .. } => {
                let t = *at.spans().first().ok_or(IntersectError::Internal)?;
                edge_face_point(arr, st, fs, (e, f), (t, xyz), remaining, meter)?;
            }
            Item::Overlap { .. } => lies = true,
            Item::Flagged { xyz, why, .. } => fs.deferred.push(Deferred {
                edge: e,
                face: f,
                why: reason_of(*why),
                xyz: *xyz,
            }),
            // A curve and a surface never meet along a curve but by an overlap.
            Item::Curve { .. } => return Err(IntersectError::Internal.into()),
        }
    }
    if lies {
        fs.on_carrier.push((e, f));
    }
    Ok(())
}

/// Edge with face: where each edge of another operand crosses a flat face's plane, or meets a
/// cylinder's or a cone's carrier, a vertex on the edge and on the face; an edge lying on the
/// plane or the carrier is held to be placed piece by piece; undecided regions are held; on any
/// other curved face, the edge is pending.
///
/// # Errors
///
/// As the crossing engine's, the curve and surface intersection's, the contact rule's,
/// [`FlatFace::place`]'s and [`RoundFace::place`]'s; [`crate::rule::CoincidenceChain`] for a new
/// vertex that cannot be held within `remaining`.
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
            if fs.rounds.get(f).is_none_or(Option::is_none) {
                // Any other curved face: the edge waits for its crossings with it.
                info_of(fs, f)?.pending.edges.push(e);
                continue;
            }
            // An edge of a face on the same carrier lies on the carrier, by what it bounds; any
            // other edge meets the carrier where the curve and surface intersection finds.
            let mut shared = false;
            for &g in fs.faces_of_edge.get(e).into_iter().flatten() {
                if coincide(arr, f, g)? {
                    shared = true;
                }
            }
            if shared {
                fs.on_carrier.push((e, f));
            } else {
                edge_round(arr, st, fs, (e, f), remaining, meter)?;
            }
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

/// The paves of the line where the planes `pa` and `pb` of two faces meet: the vertices in both
/// `first` and `second`, each face's vertices by the earlier stages (on its boundary or In), each
/// with the enclosure of its distance along the line's direction n₁ × n₂, widened by its bound.
fn section_paves(
    arr: &Arrangement,
    out: &Interfered,
    (first, second): (&[usize], &[usize]),
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

/// The grid point nearest the centre of `xyz`, and a bound that reaches every point of the box
/// from it: half the box's diagonal is at most half the sum of its sides, and the rounding to the
/// grid adds less than a step.
fn centre_of(xyz: &Box3) -> Result<(P3, Bound), NumError> {
    let ([x0, y0, z0], [x1, y1, z1]) = (coordinates(xyz.min()), coordinates(xyz.max()));
    let centre = nearest_point([
        x0 / 2.0 + x1 / 2.0,
        y0 / 2.0 + y1 / 2.0,
        z0 / 2.0 + z1 / 2.0,
    ])?;
    let reach = ((x1 - x0) + (y1 - y0) + (z1 - z0)) / 2.0 + 1.0;
    Ok((centre, Bound::of(reach).ok_or(NumError::BudgetExceeded)?))
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
    let (centre, bound) = centre_of(&region.xyz)?;
    match place_in(fs, carriers, region.face, (centre, bound), meter) {
        Ok(Some(Place::Out)) => Ok(true),
        Ok(_) | Err(BuildError::Unclassified { .. }) => Ok(false),
        Err(e) => Err(e),
    }
}

/// Faces `f1`'s and `f2`'s vertices by the earlier stages (on the boundary or In), each list made
/// once, when first needed.
fn fill_vertices(
    arr: &Arrangement,
    out: &Interfered,
    fs: &FaceStage,
    on_face: &mut [Option<Vec<usize>>],
    faces: [usize; 2],
) -> Result<(), BuildError> {
    for f in faces {
        let slot = on_face
            .get_mut(f)
            .ok_or(BuildError::Missing(Entity::Face(f)))?;
        if slot.is_none() {
            let mut list = boundary_vertices(arr, &out.paves, f);
            if let Some(info) = fs.info.get(f) {
                list.extend(info.in_vertices.iter().copied());
            }
            list.sort_unstable();
            list.dedup();
            *slot = Some(list);
        }
    }
    Ok(())
}

/// The vertices both faces hold, in index order.
fn shared_vertices(
    on_face: &[Option<Vec<usize>>],
    [f1, f2]: [usize; 2],
) -> Result<Vec<usize>, BuildError> {
    let (Some(Some(first)), Some(Some(second))) = (on_face.get(f1), on_face.get(f2)) else {
        return Err(BuildError::Missing(Entity::Face(f1)));
    };
    Ok(first
        .iter()
        .filter(|v| second.binary_search(v).is_ok())
        .copied()
        .collect())
}

/// The paves in their proven order, two that are not proven apart failing as a section's.
fn section_order(faces: [usize; 2], paves: Vec<Pave>) -> Result<Vec<Pave>, BuildError> {
    let [f1, _] = faces;
    order(f1, paves).map_err(|e| match e {
        OrderError::Undecided(p) => BuildError::SectionOrder {
            faces,
            first: p.first,
            second: p.second,
        },
        OrderError::Num(n) => n.into(),
    })
}

/// Section edges, recorded on both faces and as interferences.
fn record(
    out: &mut Interfered,
    fs: &mut FaceStage,
    (faces, curve): ([usize; 2], CurveGeom),
    blocks: Vec<PaveBlock>,
) -> Result<(), BuildError> {
    let [f1, f2] = faces;
    for block in blocks {
        let index = out.sections.len();
        out.sections.push(SectionEdge {
            faces,
            curve,
            from: block.from,
            to: block.to,
            wraps: block.wraps,
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

/// The section edges where the carriers of faces `f1` and `f2` meet, kept inside both: on the line
/// of two planes for two flat faces, and on the closed forms' curves for a face on a cylinder or a
/// cone with one on another carrier ([`round_section`]). Two faces on one carrier are noted as one
/// carrier's instead, and a pair the closed forms do not settle waits as pending.
fn section(
    arr: &Arrangement,
    out: &mut Interfered,
    fs: &mut FaceStage,
    ((f1, f2), on_face): ((usize, usize), &mut [Option<Vec<usize>>]),
    remaining: Bound,
    meter: &mut WorkMeter<'_>,
) -> Result<(), BuildError> {
    if flat_of(fs, f1)?.is_none() || flat_of(fs, f2)?.is_none() {
        // A curved face on either side: two on one carrier are one carrier's, as two faces on one
        // plane are; any other pair is met in closed form where it can be, and waits otherwise.
        let same = coincide(arr, f1, f2)?;
        if !same && round_section(arr, out, fs, ([f1, f2], on_face), remaining, meter)? {
            return Ok(());
        }
        for (f, other) in [(f1, f2), (f2, f1)] {
            let info = info_of(fs, f)?;
            if same {
                info.same_carrier.push(other);
            } else {
                info.pending.faces.push(other);
            }
        }
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
    fill_vertices(arr, out, fs, on_face, [f1, f2])?;
    let (Some(Some(first)), Some(Some(second))) = (on_face.get(f1), on_face.get(f2)) else {
        return Err(BuildError::Missing(Entity::Face(f1)));
    };
    let paves = section_paves(arr, out, (first, second), (&pa, &pb))?;
    let ordered = section_order([f1, f2], paves)?;
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
            kept.push(PaveBlock {
                edge: f1,
                from: Some(*from),
                to: Some(*to),
                wraps: false,
            });
        }
    }
    record(out, fs, ([f1, f2], CurveGeom::Line([p1, p2])), kept)
}

/// The largest magnitude a coordinate of a plane's surface may have: a double holds every whole
/// number up to it exactly.
const EXACT_WHOLE: u128 = 9_007_199_254_740_992;

/// The plane n · p + d = 0 of the table as a surface the closed forms take, c + u·a + v·b, held
/// exactly by its doubles: with k the coordinate the normal is largest in, c the point where the
/// plane crosses that axis, and a and b the whole vectors nₖ·eᵢ − nᵢ·eₖ and nₖ·eⱼ − nⱼ·eₖ,
/// each square to n. Its domain reaches past the box `lo..hi` of the carrier it is to meet (where
/// xᵢ = u·nₖ and xⱼ = v·nₖ) by an eighth of its width and a step each way, so it holds every
/// point of that carrier on the plane: the closed forms take no plane without end. `None` when a
/// number is not whole in a double, or c lies past the grid's range: the pair then waits, never
/// met on a plane that is not the face's.
fn flat_surface(plane: &Plane, (lo, hi): ([f64; 3], [f64; 3])) -> Option<SurfaceKind> {
    let n: [i128; 3] = plane.n();
    // Named types throughout, so the recursion check can read them.
    let mut k: usize = 0;
    for i in 1..3 {
        let (here, best): (i128, i128) = (*n.get(i)?, *n.get(k)?);
        if here.unsigned_abs() > best.unsigned_abs() {
            k = i;
        }
    }
    let nk: i128 = *n.get(k)?;
    if nk == 0 || plane.d().checked_rem(nk)? != 0 {
        return None;
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "the whole number is at most 2^53 in size, which a double holds exactly"
    )]
    let exact = |v: i128| (v.unsigned_abs() <= EXACT_WHOLE).then_some(v as f64);
    let at = exact(plane.d().checked_div(nk)?.checked_neg()?)?;
    if at.abs() > COORD_MAX_F64 {
        return None;
    }
    let top = exact(nk)?;
    let along = |i: usize| -> Option<[f64; 3]> {
        let side = exact(n.get(i)?.checked_neg()?)?;
        Some(core::array::from_fn(|c| {
            if c == i {
                top
            } else if c == k {
                side
            } else {
                0.0
            }
        }))
    };
    let span = |i: usize| -> Option<(f64, f64)> {
        let (a, b) = (lo.get(i)? / top, hi.get(i)? / top);
        let (a, b) = (lesser(a, b), greater(a, b));
        let margin = (b - a) / 8.0 + 1.0;
        Some((a - margin, b + margin))
    };
    let (i, j) = match k {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    };
    let origin = core::array::from_fn(|c| if c == k { at } else { 0.0 });
    FlatSurface::new(origin, along(i)?, along(j)?, (span(i)?, span(j)?))
        .ok()
        .map(SurfaceKind::Plane)
}

/// The grid's range as a double, which holds it exactly.
#[allow(
    clippy::cast_precision_loss,
    reason = "the grid's range is below 2^41, which a double holds exactly"
)]
const COORD_MAX_F64: f64 = COORD_MAX as f64;

/// A face's carrier, as the closed forms are to take it.
enum Carrier<'a> {
    /// A flat face's plane of the table.
    Flat(Plane),
    /// A cylinder's or a cone's face's own carrier.
    Round(&'a SurfaceKind),
}

/// Face `f`'s carrier: `None` for a face on any other.
fn carrier_of<'a>(
    arr: &'a Arrangement,
    fs: &FaceStage,
    f: usize,
) -> Result<Option<Carrier<'a>>, BuildError> {
    if let Some(flat) = flat_of(fs, f)? {
        return Ok(Some(Carrier::Flat(*flat.plane())));
    }
    if fs.rounds.get(f).and_then(Option::as_ref).is_none() {
        return Ok(None);
    }
    let face = arr
        .faces
        .get(f)
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    let CarrierIx::Surface(ix) = face.surface else {
        return Ok(None);
    };
    Ok(arr.carriers.surface(ix).map(Carrier::Round))
}

/// The carriers of faces `f1` and `f2` as surfaces for the closed forms, at least one a cylinder
/// or a cone: a plane made a surface over the box of the other carrier ([`flat_surface`]). `None`
/// for any other pair, a round carrier whose box is not finite, or a plane no surface holds.
fn surfaces_of(
    arr: &Arrangement,
    fs: &FaceStage,
    [f1, f2]: [usize; 2],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<(SurfaceKind, SurfaceKind)>, BuildError> {
    let (Some(first), Some(second)) = (carrier_of(arr, fs, f1)?, carrier_of(arr, fs, f2)?) else {
        return Ok(None);
    };
    Ok(match (first, second) {
        (Carrier::Round(a), Carrier::Round(b)) => Some((a.clone(), b.clone())),
        (Carrier::Flat(p), Carrier::Round(b)) => plane_over(&p, b, meter)?.map(|a| (a, b.clone())),
        (Carrier::Round(a), Carrier::Flat(p)) => plane_over(&p, a, meter)?.map(|b| (a.clone(), b)),
        (Carrier::Flat(_), Carrier::Flat(_)) => None,
    })
}

/// Plane `plane` as a surface over the box of carrier `round` ([`flat_surface`]); `None` when that
/// box is not finite or no surface holds the plane exactly.
fn plane_over(
    plane: &Plane,
    round: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<SurfaceKind>, BuildError> {
    let b = round
        .bound(round.domain(), meter)
        .map_err(IntersectError::from)?;
    let finite = b.lo.iter().chain(&b.hi).all(|v: &f64| v.is_finite());
    Ok(if finite {
        flat_surface(plane, (b.lo, b.hi))
    } else {
        None
    })
}

/// Whether a carrier is one whose meetings 12.7's closed forms decide by exact signs: a plane, a
/// sphere, a cylinder or a cone.
fn quadric(surface: &SurfaceKind) -> bool {
    matches!(
        surface,
        SurfaceKind::Plane(_)
            | SurfaceKind::Sphere(_)
            | SurfaceKind::Cylinder(_)
            | SurfaceKind::Cone(_)
    )
}

/// The curves where carriers `a` and `b` cross or touch, each whole: a circle or an ellipse over
/// its full turn, any other kind as the closed forms give it; one curve once. A curve along which
/// two quadrics touch counts: the closed forms find it only where an exact sign is zero (a plane
/// as far from a cylinder's axis as its radius, say), with no band and no gap, so the carriers
/// meet along it exactly. `None` when the closed forms leave the pair (general position, or no
/// enclosure decides), or find what this stage does not make sections of: carriers that touch at a
/// point, a touch the closed forms do not prove exact, a stretch ending at a flagged region, or an
/// open curve cut at a seam. Two curves with an empty list: the carriers are proven not to meet.
fn section_curves(
    a: &SurfaceKind,
    b: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<CurveKind<3>>>, BuildError> {
    let found = match natural(a, b, meter) {
        Ok(Some(found)) => found,
        Ok(None) | Err(IntersectError::FrameRange) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let exact = quadric(a) && quadric(b);
    let mut curves: Vec<CurveKind<3>> = Vec::new();
    for item in found.items() {
        let Item::Curve {
            curve,
            contact,
            ends,
            ..
        } = item
        else {
            return Ok(None);
        };
        let meets = match contact {
            CurveContact::Crossing => true,
            CurveContact::Tangent { band, gap } => {
                exact && band.get() == 0 && gap.lo().get() == 0 && gap.hi().get() == 0
            }
        };
        if !meets {
            return Ok(None);
        }
        if ends
            .iter()
            .any(|e| matches!(e, CurveEnd::Singular | CurveEnd::Flagged))
        {
            return Ok(None);
        }
        let turn = (0.0, 2.0);
        let curve: &CurveKind<3> = curve;
        let whole: CurveKind<3> = match curve {
            CurveKind::Circle(c) => CurveKind::Circle(
                Circle::new(c.center(), c.radius(), c.frame(), turn)
                    .map_err(IntersectError::from)?,
            ),
            CurveKind::Ellipse(e) => CurveKind::Ellipse(
                Ellipse::new(e.center(), e.axes(), turn).map_err(IntersectError::from)?,
            ),
            _ if ends.contains(&CurveEnd::Seam) => return Ok(None),
            other => other.clone(),
        };
        if !curves.contains(&whole) {
            curves.push(whole);
        }
    }
    Ok(Some(curves))
}

/// Whether a curve closes on itself: a circle or an ellipse over its full turn.
fn closes(curve: &CurveKind<3>) -> bool {
    matches!(curve, CurveKind::Circle(_) | CurveKind::Ellipse(_))
}

/// Which of `curves` vertex `vertex` (at grid point `at`) lies on, and its pave there: the curve
/// whose nearest point is proven nearer to it than any other's, at that point's parameter, as an
/// edge's paves are found. The vertex lies on both carriers, so on where they meet; the proof only
/// says which of the meeting's curves.
fn curve_pave(
    curves: &[CurveKind<3>],
    (faces, vertex, at): ([usize; 2], usize, P3),
    meter: &mut WorkMeter<'_>,
) -> Result<(usize, Pave), BuildError> {
    let refused = BuildError::SectionVertex { faces, vertex };
    let scale = (0..REFINEMENTS).fold(1.0_f64, |s, _| s * 2.0);
    let mut near: Vec<(usize, Interval, Interval)> = Vec::with_capacity(curves.len());
    for (i, curve) in curves.iter().enumerate() {
        match project_curve(curve, &coordinates(at), FIRST_FLOOR / scale, meter)? {
            Projection::Points(found) => {
                let first = found.first().ok_or(refused)?;
                let t = *first.at.first().ok_or(refused)?;
                near.push((i, t, first.distance));
            }
            Projection::Set { .. } | Projection::NonIsolated { .. } => return Err(refused),
        }
    }
    let mut best: Option<(usize, Interval, Interval)> = None;
    for &(i, t, d) in &near {
        if best.is_none_or(|(_, _, b)| d.hi() < b.hi()) {
            best = Some((i, t, d));
        }
    }
    let (nearest, t, gap) = best.ok_or(refused)?;
    if near
        .iter()
        .any(|&(other, _, far)| other != nearest && far.lo() <= gap.hi())
    {
        return Err(refused);
    }
    Ok((nearest, Pave { vertex, t }))
}

/// The apex of a cone, the point of its carrier where its radius r + k·v is nothing, at the
/// height s·v along its axis: v = −r / k. `None` for any other carrier.
fn apex_of(surface: &SurfaceKind) -> Option<[f64; 3]> {
    let SurfaceKind::Cone(cone) = surface else {
        return None;
    };
    let (radius, slope) = cone.radius_and_slope();
    if slope == 0.0 {
        return None;
    }
    let height = cone.axial_sign() * (-radius / slope);
    let ([_, _, axis], _) = cone.frame().unit_vectors().ok()?;
    let ([c0, c1, c2], [a0, a1, a2]) = (cone.center(), axis);
    Some([c0 + height * a0, c1 + height * a1, c2 + height * a2])
}

/// Each open curve of `curves` with an end at the grid point `at`, by the contact rule, with that
/// end's parameter.
///
/// # Errors
///
/// A curve's bound's; [`crate::rule::NearContact`] for an end the rule cannot decide.
fn ending_at(
    curves: &[CurveKind<3>],
    at: P3,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<(usize, Interval)>, BuildError> {
    let mut out = Vec::new();
    for (i, curve) in curves.iter().enumerate() {
        if closes(curve) {
            continue;
        }
        let (t0, t1) = curve.domain();
        for t in [t0, t1] {
            let end = curve.bound((t, t), meter).map_err(IntersectError::from)?;
            let near = to_box(at, Bound::ZERO, &box_of(end.lo, end.hi)?)?;
            match judge(near) {
                Verdict::Touch => out.push((i, Interval::point(t)?)),
                Verdict::Decided(_) => {}
                Verdict::Near => {
                    return Err(ContactError::NearContact(NearContact {
                        enclosure: near,
                        refinement: REFINEMENTS,
                    })
                    .into());
                }
            }
        }
    }
    Ok(out)
}

/// Where a cone's apex, which section curves run from, stands to two faces.
enum Apex {
    /// At this vertex of both.
    At(usize),
    /// Outside one of them: no section reaches it.
    Outside,
    /// On a face's boundary away from its vertices: the pair waits.
    Unsettled,
}

/// The vertex at a cone's apex, the grid point `at`, for the section curves of `faces` that run
/// from it: a vertex both faces hold that touches it, or else, where the apex lies inside both
/// faces, a new vertex certified within `remaining` of both carriers, In both. An apex outside
/// either face is no vertex: a stretch of a curve from the apex to its first pave meets neither
/// face's boundary, so it lies outside that face too.
///
/// # Errors
///
/// As [`crate::interfere::certify`]'s and the placing's.
fn apex_vertex(
    arr: &Arrangement,
    out: &mut Interfered,
    fs: &mut FaceStage,
    (faces, on_face): ([usize; 2], &mut [Option<Vec<usize>>]),
    (at, remaining): (P3, Bound),
    meter: &mut WorkMeter<'_>,
) -> Result<Apex, BuildError> {
    let carriers = &arr.carriers;
    let xyz = box_of(coordinates(at), coordinates(at))?;
    if let Some(v) = touching(
        carriers,
        &out.vertices,
        shared_vertices(on_face, faces)?,
        &xyz,
    )? {
        return Ok(Apex::At(v));
    }
    let mut on = Vec::with_capacity(faces.len());
    for f in faces {
        let face = arr
            .faces
            .get(f)
            .ok_or(BuildError::Missing(Entity::Face(f)))?;
        on.push(face.surface);
    }
    let point = certify(carriers, at, SortedVec::from_items(on), remaining, meter)?;
    let (_, within) = carriers.commit(&point)?;
    for f in faces {
        match place_in(fs, carriers, f, (at, within), meter)? {
            Some(Place::In) => {}
            Some(Place::Out) => return Ok(Apex::Outside),
            Some(Place::On(_)) | None => return Ok(Apex::Unsettled),
        }
    }
    out.vertices.push(point);
    let v = out.vertices.len().saturating_sub(1);
    for f in faces {
        info_of(fs, f)?.in_vertices.push(v);
        let list = on_face
            .get_mut(f)
            .and_then(Option::as_mut)
            .ok_or(BuildError::Missing(Entity::Face(f)))?;
        list.push(v);
        list.sort_unstable();
        out.interferences
            .push(Interference::VertexFace { vertex: v, face: f });
    }
    Ok(Apex::At(v))
}

/// A curved face with a face on another carrier (a plane of the table, or a quadric or torus the
/// closed forms take), not one carrier: the section edges along each curve where the carriers
/// cross in closed form ([`section_curves`]), kept where inside both faces. The paves are the
/// vertices both faces hold, each on its curve ([`curve_pave`]), and at a cone's apex, where
/// curves through it end, the vertex there ([`apex_vertex`]) on each of them; the pieces are the
/// blocks between neighbouring paves, round the seam of a closed curve, and the whole of a closed
/// curve with none. A piece meets neither face's boundary between its paves, since every crossing
/// of a boundary edge with the other face's carrier is a vertex of both, so its middle places it.
/// `false` when the closed forms leave the pair, or a cone's apex lies on a face's boundary away
/// from its vertices: it waits.
fn round_section(
    arr: &Arrangement,
    out: &mut Interfered,
    fs: &mut FaceStage,
    (faces, on_face): ([usize; 2], &mut [Option<Vec<usize>>]),
    remaining: Bound,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, BuildError> {
    let [f1, f2] = faces;
    let Some((a, b)) = surfaces_of(arr, fs, faces, meter)? else {
        return Ok(false);
    };
    let Some(curves) = section_curves(&a, &b, meter)? else {
        return Ok(false);
    };
    fill_vertices(arr, out, fs, on_face, faces)?;
    // The curves that run from a cone's apex, and the vertex there.
    let mut ending = Vec::new();
    let mut apex = None;
    if let Some(at) = apex_of(&a).or_else(|| apex_of(&b)) {
        let at = nearest_point(at)?;
        ending = ending_at(&curves, at, meter)?;
        if !ending.is_empty() {
            match apex_vertex(arr, out, fs, (faces, on_face), (at, remaining), meter)? {
                Apex::At(v) => apex = Some(v),
                Apex::Outside => {}
                Apex::Unsettled => return Ok(false),
            }
        }
    }
    let mut paves: Vec<Vec<Pave>> = alloc::vec![Vec::new(); curves.len()];
    for v in shared_vertices(on_face, faces)? {
        if apex == Some(v) {
            continue;
        }
        let (p, _) = committed(&arr.carriers, &out.vertices, v)?;
        let (i, pave) = curve_pave(&curves, (faces, v, p), meter)?;
        paves.get_mut(i).ok_or(IntersectError::Internal)?.push(pave);
    }
    if let Some(vertex) = apex {
        for (i, t) in ending {
            paves
                .get_mut(i)
                .ok_or(IntersectError::Internal)?
                .push(Pave { vertex, t });
        }
    }
    let carriers = &arr.carriers;
    for (curve, list) in curves.into_iter().zip(paves) {
        let ordered = section_order(faces, list)?;
        let mut kept = Vec::new();
        for block in blocks(f1, &ordered, closes(&curve)) {
            let Some(t) = middle_of(&curve, &block) else {
                continue;
            };
            let middle = near_point(&curve, t, meter)?;
            if place_in(fs, carriers, f1, middle, meter)? == Some(Place::In)
                && place_in(fs, carriers, f2, middle, meter)? == Some(Place::In)
            {
                kept.push(block);
            }
        }
        if kept.is_empty() {
            continue;
        }
        let ix = out.curves.len();
        out.curves.push(curve);
        record(out, fs, (faces, CurveGeom::Section(ix)), kept)?;
    }
    Ok(true)
}

/// The edges held as lying on a face's plane, or on a curved face's carrier, placed piece by
/// piece: each split edge of one whose middle lies inside the face is In it. A middle outside a
/// flat face's box is placed Out by [`FlatFace::place`]'s own box test, at once.
fn lying(
    arr: &Arrangement,
    out: &Interfered,
    fs: &mut FaceStage,
    by_source: &[Vec<usize>],
    meter: &mut WorkMeter<'_>,
) -> Result<(), BuildError> {
    let carriers = &arr.carriers;
    let mut held = core::mem::take(&mut fs.on_plane);
    held.append(&mut fs.on_carrier);
    for (e, f) in held {
        let edge = arr
            .edges
            .get(e)
            .ok_or(BuildError::Missing(Entity::Edge(e)))?;
        let curve = carriers
            .curve(edge.curve)
            .ok_or(BuildError::Missing(Entity::Edge(e)))?;
        let mut inside = Vec::new();
        for &k in by_source.get(e).into_iter().flatten() {
            let Some(piece) = out.splits.get(k) else {
                continue;
            };
            let Some(t) = middle_of(curve, &piece.block) else {
                continue;
            };
            let (p, w) = near_point(curve, t, meter)?;
            if place_in(fs, carriers, f, (p, w), meter)? == Some(Place::In) {
                inside.push(k);
            }
        }
        info_of(fs, f)?.in_blocks.extend(inside);
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
    remaining: Bound,
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
    lying(arr, out, &mut fs, &by_source, meter)?;
    // Each face's vertices, by the earlier stages (on its boundary or In), made once when needed.
    let mut on_face: Vec<Option<Vec<usize>>> = alloc::vec![None; arr.faces.len()];
    for &pair in face_pairs {
        section(arr, out, &mut fs, (pair, &mut on_face), remaining, meter)?;
    }
    for (f, face) in arr.faces.iter().enumerate() {
        let blocks: Vec<usize> = face
            .loops
            .iter()
            .flatten()
            .filter_map(|u| by_source.get(u.edge))
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::float_cmp,
    reason = "test code: a failure should stop loudly"
)]
mod tests {
    use super::*;
    use geom_curve::{Circle, Frame, Line};
    use geom_run::Budget;

    fn pave(t: f64) -> Pave {
        Pave {
            vertex: 0,
            t: Interval::point(t).unwrap(),
        }
    }

    fn block(from: Option<f64>, to: Option<f64>, wraps: bool) -> PaveBlock {
        PaveBlock {
            edge: 0,
            from: from.map(pave),
            to: to.map(pave),
            wraps,
        }
    }

    #[test]
    fn a_box_s_centre_and_reach() {
        let (centre, bound) =
            centre_of(&Box3::new(P3::new(0, 0, 0).unwrap(), P3::new(4, 2, 6).unwrap()).unwrap())
                .unwrap();
        assert_eq!(centre, P3::new(2, 1, 3).unwrap());
        assert_eq!(bound, Bound::of(7.0).unwrap());
        // Halves round to even, and the reach covers the box's corners.
        let (centre, bound) =
            centre_of(&Box3::new(P3::new(-3, 1, 5).unwrap(), P3::new(-2, 2, 6).unwrap()).unwrap())
                .unwrap();
        assert_eq!(centre, P3::new(-2, 2, 6).unwrap());
        assert_eq!(bound, Bound::of(2.5).unwrap());
    }

    #[test]
    fn a_block_s_middle_lies_between_its_paves() {
        let line = Line::new([0.0; 3], [10.0, 0.0, 0.0], (0.0, 1.0)).unwrap();
        assert_eq!(
            middle_of(&line, &block(Some(0.25), Some(0.75), false)),
            Some(0.5)
        );
        assert_eq!(middle_of(&line, &block(Some(0.5), Some(0.5), false)), None);
        assert_eq!(middle_of(&line, &block(None, None, false)), Some(0.5));
        let circle = Circle::new([0.0; 3], 100.0, Frame::standard().unwrap(), (0.0, 2.0)).unwrap();
        // Round the seam: from 1.5 to the end, or from the start to the last pave.
        assert_eq!(
            middle_of(&circle, &block(Some(1.5), Some(0.5), true)),
            Some(1.75)
        );
        assert_eq!(
            middle_of(&circle, &block(Some(2.0), Some(0.5), true)),
            Some(0.25)
        );
    }

    #[test]
    fn a_point_near_a_curve_with_its_reach() {
        let m = &mut WorkMeter::new(Budget::UNLIMITED);
        let line = Line::new([0.0; 3], [10.0, 0.0, 0.0], (0.0, 1.0)).unwrap();
        // Exactly on a grid point: the rounding's step alone.
        let (p, w) = near_point(&line, 0.5, m).unwrap();
        assert_eq!(p, P3::new(5, 0, 0).unwrap());
        assert_eq!(w, Bound::of(1.0).unwrap());
        // A circle's point is enclosed with some width: a step more.
        let circle = Circle::new([0.0; 3], 1000.0, Frame::standard().unwrap(), (0.0, 2.0)).unwrap();
        let (p, w) = near_point(&circle, 0.25, m).unwrap();
        assert_eq!(p, P3::new(707, 707, 0).unwrap());
        assert_eq!(w, Bound::of(2.0).unwrap());
    }
}
