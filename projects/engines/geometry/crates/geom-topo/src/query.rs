//! Core queries on a body's topology and points (10.2, 22; piece G102): its faces listed, its
//! bounding box, and the exact mass properties of a flat body whose vertices are grid points.
//!
//! What it does: [`faces`] lists every face with its shell, surface, sense, loops and name.
//! [`loop_area`] and [`face_area`] give twice the area vector of a loop and of a face, Σ pᵢ × pᵢ₊₁
//! over the loop's vertices in fin order; [`shell_volume`] and [`volume`] give six times the signed
//! volume enclosed, Σ over faces of the face's twice-area vector dotted with one of its vertices.
//! [`bounding_box`] gives the box of every vertex, joined with the stored box of each face that is
//! not a polygon. A loop's vertex at each fin is where the fin starts: its edge's start for a
//! forward fin, its end for a reversed one.
//!
//! Guarantees:
//!
//! - **Exact.** Integers throughout, never rounded: an area vector's components are `i128` (each
//!   below n·2⁸¹ for a loop of n fins, so below 2¹¹⁴ for any arena), a volume is an `I256` (below
//!   2¹⁸⁷). A face's area vector is normal to its plane and points the way its loops run
//!   counter-clockwise; with outer loops counter-clockwise seen from outside the material, a closed
//!   solid shell's volume is positive and a cavity's shell's negative.
//! - **Flat only, said so.** A loop with an edge that is not a straight line between two vertices
//!   is refused with [`QueryError::Curved`] naming the edge, a face not on a plane with
//!   [`QueryError::NotPlanar`], a face with no loop with [`QueryError::Loopless`], and the volume of
//!   a body that is not a solid with [`QueryError::NotSolid`]. Certified mass properties of curved
//!   bodies and of implicit vertices come with later pieces.
//! - **Ends on any body.** Every list and ring walk is bounded by its arena's length, so a ring that
//!   does not come back is [`QueryError::Ring`], never a hang; a link out of its arena is
//!   [`QueryError::NoSuchRow`], never a panic.
//! - **Charged.** One unit of work per face listed, per vertex boxed and per loop, face and fin
//!   visited.
//!
//! Method: the vector area of a polygon in space and a closed polyhedron's volume by the divergence
//! theorem over its faces, `6V = Σ A_f · p_f` (Goldman, "Area of planar polygons and volume of
//! polyhedra", Graphics Gems II, 1991, pp. 170–171, doi:10.1016/B978-0-08-050754-5.50043-8).
//!
//! Cost: linear in the rows visited; each list is walked once per query, and memory holds one
//! loop's vertices at a time.

use alloc::vec::Vec;
use core::fmt;

use geom_curve::CurveKind;
use geom_math::Box3;
use geom_num::grid::P3;
use geom_num::wide::I256;
use geom_run::{Stopped, WorkMeter};
use geom_surface::SurfaceKind;

use crate::body::{Body, BodyKind};
use crate::entity::{EdgeIx, FaceIx, FinIx, LoopIx, Sense, ShellIx, SurfaceIx, VertexIx};
use crate::order::NameKey;

/// Why a query could not answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryError {
    /// A link into a row its arena does not have; a published body never holds one.
    NoSuchRow,
    /// A loop's ring of fins, or a list of loops or faces, that does not come back or end within
    /// its arena.
    Ring,
    /// An edge that is not a straight line between two vertices, where a polygon is needed.
    Curved(EdgeIx),
    /// A face whose surface is not a plane, where a polygon is needed.
    NotPlanar(FaceIx),
    /// A face with no loop, where a polygon is needed.
    Loopless(FaceIx),
    /// A volume asked of a body that is not a solid.
    NotSolid,
    /// A sum past its integer's width: never for a body within its arenas' limits.
    Overflow,
    /// The meter stopped the work.
    Stopped(Stopped),
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryError::NoSuchRow => f.write_str("a link into a row its arena does not have"),
            QueryError::Ring => f.write_str("a list or ring that does not end within its arena"),
            QueryError::Curved(e) => write!(
                f,
                "edge {} is not a straight line between two vertices",
                e.row()
            ),
            QueryError::NotPlanar(face) => write!(f, "face {} is not on a plane", face.row()),
            QueryError::Loopless(face) => write!(f, "face {} has no loop", face.row()),
            QueryError::NotSolid => f.write_str("the body is not a solid"),
            QueryError::Overflow => f.write_str("a sum past its integer's width"),
            QueryError::Stopped(s) => write!(f, "{s}"),
        }
    }
}

impl core::error::Error for QueryError {}

impl From<Stopped> for QueryError {
    fn from(s: Stopped) -> Self {
        QueryError::Stopped(s)
    }
}

/// One face, as [`faces`] lists it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaceRow {
    /// The face.
    pub face: FaceIx,
    /// Its shell.
    pub shell: ShellIx,
    /// Its surface.
    pub surface: SurfaceIx,
    /// Which side of its surface the material is on.
    pub sense: Sense,
    /// How many loops bound it.
    pub loops: u32,
    /// Its persistent name, for a body published by name.
    pub name: Option<NameKey>,
}

/// The loops of `face`, in list order, each charged one unit.
///
/// # Errors
///
/// [`QueryError::NoSuchRow`] for a link out of its arena, [`QueryError::Ring`] for a list or ring
/// that does not end within it, and the meter's stop.
pub fn face_loops(
    body: &Body,
    face: FaceIx,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<LoopIx>, QueryError> {
    let a = body.arenas();
    let f = a.faces.get(face).ok_or(QueryError::NoSuchRow)?;
    let mut out = Vec::new();
    let mut at = f.first_loop;
    while let Some(l) = at {
        if out.len() >= a.loops.len() as usize {
            return Err(QueryError::Ring);
        }
        meter.charge(1)?;
        out.push(l);
        at = a.loops.get(l).ok_or(QueryError::NoSuchRow)?.next;
    }
    Ok(out)
}

/// The fins of loop `l`, in ring order from its first, each charged one unit.
///
/// # Errors
///
/// [`QueryError::NoSuchRow`] for a link out of its arena, [`QueryError::Ring`] for a list or ring
/// that does not end within it, and the meter's stop.
pub fn loop_fins(
    body: &Body,
    l: LoopIx,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<FinIx>, QueryError> {
    let a = body.arenas();
    let Some(first) = a.loops.get(l).ok_or(QueryError::NoSuchRow)?.first_fin else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    let mut at = first;
    loop {
        if out.len() >= a.fins.len() as usize {
            return Err(QueryError::Ring);
        }
        meter.charge(1)?;
        out.push(at);
        at = a.fins.get(at).ok_or(QueryError::NoSuchRow)?.next;
        if at == first {
            return Ok(out);
        }
    }
}

/// The point of vertex `v`.
///
/// # Errors
///
/// [`QueryError::NoSuchRow`] for a vertex or point out of its arena.
pub fn point(body: &Body, v: VertexIx) -> Result<P3, QueryError> {
    let a = body.arenas();
    let vertex = a.vertices.get(v).ok_or(QueryError::NoSuchRow)?;
    a.points.get(vertex.point).copied().ok_or(QueryError::NoSuchRow)
}

/// The vertex fin `fin` starts at, if its edge is a straight line between two vertices.
fn polygon_vertex(body: &Body, fin: FinIx) -> Result<VertexIx, QueryError> {
    let a = body.arenas();
    let f = a.fins.get(fin).ok_or(QueryError::NoSuchRow)?;
    let e = a.edges.get(f.edge).ok_or(QueryError::NoSuchRow)?;
    let line = matches!(a.curves.get(e.curve), Some(CurveKind::Line(_)));
    match (line, e.start, e.end, f.sense) {
        (true, Some(start), Some(_), Sense::Forward) => Ok(start),
        (true, Some(_), Some(end), Sense::Reversed) => Ok(end),
        _ => Err(QueryError::Curved(f.edge)),
    }
}

/// The points of polygon loop `l`, in ring order.
///
/// # Errors
///
/// [`QueryError::Curved`] for an edge that is not a straight line between two vertices, and the
/// walk's and the meter's errors.
pub fn loop_points(
    body: &Body,
    l: LoopIx,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<P3>, QueryError> {
    let mut out = Vec::new();
    for fin in loop_fins(body, l, meter)? {
        out.push(point(body, polygon_vertex(body, fin)?)?);
    }
    Ok(out)
}

/// `p × q`, exactly.
fn cross(p: P3, q: P3) -> [i128; 3] {
    let [px, py, pz] = [p.x, p.y, p.z].map(|c| i128::from(c.get()));
    let [qx, qy, qz] = [q.x, q.y, q.z].map(|c| i128::from(c.get()));
    // Each coordinate is below 2^40 in size, each product below 2^80 and each difference below
    // 2^81, far inside i128.
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "bounded above: each difference is below 2^81"
    )]
    [py * qz - pz * qy, pz * qx - px * qz, px * qy - py * qx]
}

/// `a + b`, component by component.
fn add(a: [i128; 3], b: [i128; 3]) -> Result<[i128; 3], QueryError> {
    let [ax, ay, az] = a;
    let [bx, by, bz] = b;
    Ok([
        ax.checked_add(bx).ok_or(QueryError::Overflow)?,
        ay.checked_add(by).ok_or(QueryError::Overflow)?,
        az.checked_add(bz).ok_or(QueryError::Overflow)?,
    ])
}

/// Twice the area vector of the polygon through `points`, in order and closed.
fn polygon_area(points: &[P3]) -> Result<[i128; 3], QueryError> {
    let mut sum = [0; 3];
    for (p, q) in points.iter().zip(points.iter().cycle().skip(1)) {
        sum = add(sum, cross(*p, *q))?;
    }
    Ok(sum)
}

/// Twice the area vector of loop `l`: Σ pᵢ × pᵢ₊₁ over its vertices in ring order.
///
/// # Errors
///
/// [`QueryError::Curved`] for an edge that is not a line between two vertices, and the walk's and
/// the meter's errors.
pub fn loop_area(
    body: &Body,
    l: LoopIx,
    meter: &mut WorkMeter<'_>,
) -> Result<[i128; 3], QueryError> {
    polygon_area(&loop_points(body, l, meter)?)
}

/// A flat face's twice-area vector and the first vertex of its first loop.
fn face_polygon(
    body: &Body,
    face: FaceIx,
    meter: &mut WorkMeter<'_>,
) -> Result<([i128; 3], P3), QueryError> {
    let a = body.arenas();
    let f = a.faces.get(face).ok_or(QueryError::NoSuchRow)?;
    if !matches!(a.surfaces.get(f.surface), Some(SurfaceKind::Plane(_))) {
        return Err(QueryError::NotPlanar(face));
    }
    let mut sum = [0; 3];
    let mut first = None;
    for l in face_loops(body, face, meter)? {
        let points = loop_points(body, l, meter)?;
        first = first.or(points.first().copied());
        sum = add(sum, polygon_area(&points)?)?;
    }
    Ok((sum, first.ok_or(QueryError::Loopless(face))?))
}

/// Twice the area vector of `face`: the sum of its loops' (holes run the other way round, so they
/// subtract).
///
/// # Errors
///
/// [`QueryError::NotPlanar`] for a face not on a plane, [`QueryError::Loopless`] for one with no
/// vertex in its loops, [`QueryError::Curved`] for an edge that is not a line between two vertices,
/// and the walk's and the meter's errors.
pub fn face_area(
    body: &Body,
    face: FaceIx,
    meter: &mut WorkMeter<'_>,
) -> Result<[i128; 3], QueryError> {
    Ok(face_polygon(body, face, meter)?.0)
}

/// The faces of shell `s`, in list order, each charged one unit.
///
/// # Errors
///
/// [`QueryError::NoSuchRow`] for a link out of its arena, [`QueryError::Ring`] for a list or ring
/// that does not end within it, and the meter's stop.
pub fn shell_faces(
    body: &Body,
    s: ShellIx,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<FaceIx>, QueryError> {
    let a = body.arenas();
    let mut out = Vec::new();
    let mut at = a.shells.get(s).ok_or(QueryError::NoSuchRow)?.first_face;
    while let Some(f) = at {
        if out.len() >= a.faces.len() as usize {
            return Err(QueryError::Ring);
        }
        meter.charge(1)?;
        out.push(f);
        at = a.faces.get(f).ok_or(QueryError::NoSuchRow)?.next;
    }
    Ok(out)
}

/// Six times the signed volume shell `s` encloses: Σ over its faces of `A_f · p_f`, `A_f` a face's
/// twice-area vector and `p_f` a vertex of it.
///
/// # Errors
///
/// [`QueryError::NotPlanar`], [`QueryError::Loopless`] and [`QueryError::Curved`] for a face that is
/// not a polygon, and the walk's and the meter's errors.
pub fn shell_volume(
    body: &Body,
    s: ShellIx,
    meter: &mut WorkMeter<'_>,
) -> Result<I256, QueryError> {
    let mut sum = I256::from_i64(0);
    for face in shell_faces(body, s, meter)? {
        let (area, p) = face_polygon(body, face, meter)?;
        for (c, x) in area.into_iter().zip([p.x, p.y, p.z]) {
            let term = I256::from_i128(c)
                .checked_mul(&I256::from_i64(x.get()))
                .ok_or(QueryError::Overflow)?;
            sum = sum.checked_add(&term).ok_or(QueryError::Overflow)?;
        }
    }
    Ok(sum)
}

/// Six times the volume of a solid body: the sum over all its shells, a cavity's counting
/// negative.
///
/// # Errors
///
/// [`QueryError::NotSolid`] for a body of another kind, and [`shell_volume`]'s errors.
pub fn volume(body: &Body, meter: &mut WorkMeter<'_>) -> Result<I256, QueryError> {
    if body.kind() != BodyKind::Solid {
        return Err(QueryError::NotSolid);
    }
    let mut sum = I256::from_i64(0);
    for row in 0..body.arenas().shells.len() {
        let s = ShellIx::new(row).ok_or(QueryError::NoSuchRow)?;
        sum = sum
            .checked_add(&shell_volume(body, s, meter)?)
            .ok_or(QueryError::Overflow)?;
    }
    Ok(sum)
}

/// Every face of `body`, in arena order.
///
/// # Errors
///
/// The walk's and the meter's errors.
pub fn faces(body: &Body, meter: &mut WorkMeter<'_>) -> Result<Vec<FaceRow>, QueryError> {
    let a = body.arenas();
    let names = body.names();
    let mut out = Vec::new();
    for row in 0..a.faces.len() {
        meter.charge(1)?;
        let face = FaceIx::new(row).ok_or(QueryError::NoSuchRow)?;
        let f = a.faces.get(face).ok_or(QueryError::NoSuchRow)?;
        let loops = face_loops(body, face, meter)?;
        out.push(FaceRow {
            face,
            shell: f.shell,
            surface: f.surface,
            sense: f.sense,
            loops: u32::try_from(loops.len()).map_err(|_| QueryError::Ring)?,
            name: names.and_then(|n| n.faces.get(face.to_usize()).copied()),
        });
    }
    Ok(out)
}

/// The bounding box of `body` and whether it is tight: the box of every vertex, joined with the
/// stored box of every face that is not a polygon on a plane, so tight exactly when every face is
/// one; `None` for a body with neither.
///
/// # Errors
///
/// The walk's and the meter's errors.
pub fn bounding_box(
    body: &Body,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<(Box3, bool)>, QueryError> {
    let a = body.arenas();
    let mut boxed: Option<Box3> = None;
    let mut tight = true;
    let join = |b: Option<Box3>, other: Box3| Some(b.map_or(other, |x| x.union(&other)));
    for row in 0..a.vertices.len() {
        meter.charge(1)?;
        let p = point(body, VertexIx::new(row).ok_or(QueryError::NoSuchRow)?)?;
        boxed = join(boxed, Box3::new(p, p).map_err(|_| QueryError::NoSuchRow)?);
    }
    for row in 0..a.faces.len() {
        let face = FaceIx::new(row).ok_or(QueryError::NoSuchRow)?;
        match face_polygon(body, face, meter) {
            Ok(_) => {}
            Err(QueryError::NotPlanar(_) | QueryError::Loopless(_) | QueryError::Curved(_)) => {
                tight = false;
                boxed = join(boxed, a.faces.get(face).ok_or(QueryError::NoSuchRow)?.bound);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(boxed.map(|b| (b, tight)))
}
