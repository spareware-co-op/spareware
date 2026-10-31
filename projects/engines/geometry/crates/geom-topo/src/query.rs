//! Core queries on a body's topology and points (10.2, 22; piece G102): its faces listed, its
//! bounding box, the exact mass properties of a flat body whose vertices are grid points, and which
//! side of a flat shell a point or another shell lies on.
//!
//! What it does: [`faces`] lists every face with its shell, surface, sense, loops and name.
//! [`loop_area`] and [`face_area`] give twice the area vector of a loop and of a face, Σ pᵢ × pᵢ₊₁
//! over the loop's vertices in fin order; [`shell_volume`] and [`volume`] give six times the signed
//! volume enclosed, Σ over faces of the face's twice-area vector dotted with one of its vertices.
//! [`bounding_box`] gives the box of every vertex, joined with the stored box of each face that is
//! not a polygon. [`side_of_shell`] says which side of a closed flat shell a grid point lies on,
//! and [`shell_side`] which side of one shell another lies on. A loop's vertex at each fin is where
//! the fin starts: its edge's start for a forward fin, its end for a reversed one.
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
//!   bodies come with later pieces, and those of implicit vertices are `crate::mass`'s.
//! - **Exact sides, every tie decided.** [`side_of_shell`] is a signed count of a ray's crossings
//!   with the fan triangles of the shell's loops, decided by `orient2d` and `orient3d` on grid
//!   points, with the query point moved symbolically so that no tie is left. It is the point's
//!   winding number, so a shell inside out, a face with holes and a loop that is not convex all
//!   count correctly. A point at one of the shell's vertices is reported as such, never classified.
//! - **Ends on any body.** Every list and ring walk is bounded by its arena's length, so a ring that
//!   does not come back is [`QueryError::Ring`], never a hang; a link out of its arena is
//!   [`QueryError::NoSuchRow`], never a panic.
//! - **Charged, and nothing allocated but one list.** One unit of work per face listed, per vertex
//!   boxed and per loop, face and fin visited. Only [`faces`] allocates: its list, reserved
//!   fallibly with its logical bytes on the meter first.
//!
//! Method: the vector area of a polygon in space and a closed polyhedron's volume by the divergence
//! theorem over its faces, `6V = Σ A_f · p_f` (Goldman, "Area of planar polygons and volume of
//! polyhedra", Graphics Gems II, 1991, pp. 170–171, doi:10.1016/B978-0-08-050754-5.50043-8). The
//! side of a shell is the winding number by signed crossings (Feito and Torres, "Inclusion test for
//! general polyhedra", Computers and Graphics 21(1), 1997, doi:10.1016/S0097-8493(96)00028-8), on a
//! ray along +x from p + (ε³, ε, ε²): Simulation of Simplicity's idea (Edelsbrunner and Mücke 1990,
//! doi:10.1145/77635.77639) applied to the query point alone. `geom_num::perturb` is not used
//! because its form of the test needs a far end of the ray inside the grid, which a shell filling
//! the grid's range leaves no room for, and its 2D and 3D answers are signs of different
//! perturbations; moving one point needs only the leading terms written at `crossing`.
//!
//! Cost: linear in the rows visited; each list is walked once per query. [`shell_side`] is one
//! [`side_of_shell`] for each vertex of the first shell the second holds, plus one. Recursion:
//! none.

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;

use geom_curve::CurveKind;
use geom_math::Box3;
use geom_num::grid::{P2, P3};
use geom_num::predicate::{orient2d, orient3d};
use geom_num::wide::I256;
use geom_num::{NumError, Sign};
use geom_op::run::reserve;
use geom_run::{LogicalSize, Stopped, WorkMeter};
use geom_surface::SurfaceKind;

use crate::body::{Body, BodyKind};
use crate::entity::{
    EdgeIx, Face, FaceIx, Fin, FinIx, Loop, LoopIx, Sense, ShellIx, SurfaceIx, VertexIx,
};
use crate::ix::Ix;
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
    /// A sum past its integer's width, or an exact predicate that could not finish: never for a
    /// body within its arenas' limits.
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

impl From<NumError> for QueryError {
    fn from(_: NumError) -> Self {
        QueryError::Overflow
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

impl LogicalSize for FaceRow {
    /// Five 4-byte fields and a name of 16 bytes.
    const LOGICAL_BYTES: u64 = 36;
}

/// Where a point lies against a closed shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Inside what the shell encloses, whichever way its faces point.
    Inside,
    /// Outside it.
    Outside,
}

/// A walk along a list of rows, ended by its arena's length.
struct Walk<K> {
    at: Option<Ix<K>>,
    left: u32,
}

impl<K> Walk<K> {
    /// The row the walk is at, charged one unit.
    fn take(&mut self, meter: &mut WorkMeter<'_>) -> Result<Option<Ix<K>>, QueryError> {
        let Some(at) = self.at else {
            return Ok(None);
        };
        self.left = self.left.checked_sub(1).ok_or(QueryError::Ring)?;
        meter.charge(1)?;
        Ok(Some(at))
    }
}

/// The walk along `face`'s loops.
fn loops_of(body: &Body, face: FaceIx) -> Result<Walk<Loop>, QueryError> {
    let a = body.arenas();
    Ok(Walk {
        at: a.faces.get(face).ok_or(QueryError::NoSuchRow)?.first_loop,
        left: a.loops.len(),
    })
}

/// The next loop of `walk`.
fn next_loop(
    body: &Body,
    walk: &mut Walk<Loop>,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<LoopIx>, QueryError> {
    let Some(l) = walk.take(meter)? else {
        return Ok(None);
    };
    walk.at = body.arenas().loops.get(l).ok_or(QueryError::NoSuchRow)?.next;
    Ok(Some(l))
}

/// The walk along `shell`'s faces.
fn faces_of(body: &Body, shell: ShellIx) -> Result<Walk<Face>, QueryError> {
    let a = body.arenas();
    Ok(Walk {
        at: a.shells.get(shell).ok_or(QueryError::NoSuchRow)?.first_face,
        left: a.faces.len(),
    })
}

/// The next face of `walk`.
fn next_face(
    body: &Body,
    walk: &mut Walk<Face>,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<FaceIx>, QueryError> {
    let Some(f) = walk.take(meter)? else {
        return Ok(None);
    };
    walk.at = body.arenas().faces.get(f).ok_or(QueryError::NoSuchRow)?.next;
    Ok(Some(f))
}

/// A walk round a loop's ring of fins, from its first fin back to it.
struct Ring {
    walk: Walk<Fin>,
    first: Option<FinIx>,
}

/// The walk round loop `l`'s fins.
fn ring_of(body: &Body, l: LoopIx) -> Result<Ring, QueryError> {
    let a = body.arenas();
    let first = a.loops.get(l).ok_or(QueryError::NoSuchRow)?.first_fin;
    Ok(Ring {
        walk: Walk {
            at: first,
            left: a.fins.len(),
        },
        first,
    })
}

/// The next fin of `ring`.
fn next_fin(
    body: &Body,
    ring: &mut Ring,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<FinIx>, QueryError> {
    let Some(fin) = ring.walk.take(meter)? else {
        return Ok(None);
    };
    let next = body.arenas().fins.get(fin).ok_or(QueryError::NoSuchRow)?.next;
    ring.walk.at = Some(next).filter(|&n| Some(n) != ring.first);
    Ok(Some(fin))
}

/// The point of vertex `v`.
///
/// # Errors
///
/// [`QueryError::NoSuchRow`] for a vertex or point out of its arena.
pub fn point(body: &Body, v: VertexIx) -> Result<P3, QueryError> {
    let a = body.arenas();
    let vertex = a.vertices.get(v).ok_or(QueryError::NoSuchRow)?;
    a.points
        .get(vertex.point)
        .copied()
        .ok_or(QueryError::NoSuchRow)
}

/// The point fin `fin` starts at, if its edge is a straight line between two vertices.
fn polygon_point(body: &Body, fin: FinIx) -> Result<P3, QueryError> {
    let a = body.arenas();
    let f = a.fins.get(fin).ok_or(QueryError::NoSuchRow)?;
    let e = a.edges.get(f.edge).ok_or(QueryError::NoSuchRow)?;
    let line = matches!(a.curves.get(e.curve), Some(CurveKind::Line(_)));
    match (line, e.start, e.end, f.sense) {
        (true, Some(start), Some(_), Sense::Forward) => point(body, start),
        (true, Some(_), Some(end), Sense::Reversed) => point(body, end),
        _ => Err(QueryError::Curved(f.edge)),
    }
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

/// Twice the area vector of loop `l` and the point it starts at, `None` for a loop with no fins.
fn loop_polygon(
    body: &Body,
    l: LoopIx,
    meter: &mut WorkMeter<'_>,
) -> Result<([i128; 3], Option<P3>), QueryError> {
    let mut ring = ring_of(body, l)?;
    let mut sum = [0; 3];
    let mut ends: Option<(P3, P3)> = None;
    while let Some(fin) = next_fin(body, &mut ring, meter)? {
        let p = polygon_point(body, fin)?;
        ends = Some(match ends {
            None => (p, p),
            Some((first, last)) => {
                sum = add(sum, cross(last, p))?;
                (first, p)
            }
        });
    }
    if let Some((first, last)) = ends {
        sum = add(sum, cross(last, first))?;
    }
    Ok((sum, ends.map(|(first, _)| first)))
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
    Ok(loop_polygon(body, l, meter)?.0)
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
    let mut walk = loops_of(body, face)?;
    while let Some(l) = next_loop(body, &mut walk, meter)? {
        let (area, start) = loop_polygon(body, l, meter)?;
        first = first.or(start);
        sum = add(sum, area)?;
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
    let mut walk = faces_of(body, s)?;
    while let Some(face) = next_face(body, &mut walk, meter)? {
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

/// The sign of an ordering.
const fn sign(order: Ordering) -> Sign {
    match order {
        Ordering::Less => Sign::Negative,
        Ordering::Equal => Sign::Zero,
        Ordering::Greater => Sign::Positive,
    }
}

/// `orient2d(s, t, q)` for `q` moved to `q` + (ε, ε²): `orient2d` is
/// (t.x − s.x)(q.y − s.y) − (t.y − s.y)(q.x − s.x), so a tie falls to the coefficient of ε,
/// −(t.y − s.y), and then to that of ε², t.x − s.x. The caller's `s` and `t` differ, so one of
/// them is not zero.
fn side(s: P2, t: P2, q: P2) -> Result<Sign, NumError> {
    Ok(match orient2d(s, t, q)? {
        Sign::Zero if s.y != t.y => sign(s.y.get().cmp(&t.y.get())),
        Sign::Zero => sign(t.x.get().cmp(&s.x.get())),
        found => found,
    })
}

/// How the ray along +x from `p` + (ε³, ε, ε²) crosses triangle `a`, `b`, `c`: +1 going the way
/// its right-hand normal N = (b − a) × (c − a) points, −1 against it, 0 if it misses or runs beside
/// the triangle.
///
/// The triangle projected along x has twice the area N.x, so a zero is a triangle the ray runs
/// beside. Otherwise the moved point is inside the projection when it lies on the same side of all
/// three edges as the projection turns. Then the sign of N · (p' − a) says which side of the plane
/// p' is on: `orient3d`, and in a tie the first of N.y, N.z and N.x that is not zero, the
/// coefficients of ε, ε² and ε³. The ray meets the plane ahead of p' when that sign is not N.x's.
fn crossing(p: P3, a: P3, b: P3, c: P3) -> Result<i64, NumError> {
    let yz = |q: P3| P2 { x: q.y, y: q.z };
    let zx = |q: P3| P2 { x: q.z, y: q.x };
    let xy = |q: P3| P2 { x: q.x, y: q.y };
    let turn = orient2d(yz(a), yz(b), yz(c))?;
    if turn == Sign::Zero {
        return Ok(0);
    }
    for (s, t) in [(a, b), (b, c), (c, a)] {
        if side(yz(s), yz(t), yz(p))? != turn {
            return Ok(0);
        }
    }
    let mut plane = orient3d(a, b, c, p)?;
    if plane == Sign::Zero {
        plane = orient2d(zx(a), zx(b), zx(c))?;
    }
    if plane == Sign::Zero {
        plane = orient2d(xy(a), xy(b), xy(c))?;
    }
    if plane == Sign::Zero || plane == turn {
        return Ok(0);
    }
    Ok(if turn == Sign::Positive { 1 } else { -1 })
}

/// Which side of closed flat shell `s` the point `p` lies on, or `None` if a vertex of the shell
/// is at `p`: there a side would say only which way the point was moved.
///
/// The shell's loops, each a fan of triangles from its first vertex, make one closed oriented
/// surface, so the signed count of the ray's crossings is the point's winding number whichever
/// loops are holes and however they turn. A point on a face away from its vertices is classified
/// as the moved point is, so a caller must know the shell does not pass through it: the checker
/// asks only of bodies group 3 has passed.
///
/// # Errors
///
/// [`QueryError::Curved`] for an edge that is not a line between two vertices, and the walk's and
/// the meter's errors.
pub fn side_of_shell(
    body: &Body,
    s: ShellIx,
    p: P3,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Side>, QueryError> {
    let mut count = 0_i64;
    let mut faces = faces_of(body, s)?;
    while let Some(face) = next_face(body, &mut faces, meter)? {
        let mut loops = loops_of(body, face)?;
        while let Some(l) = next_loop(body, &mut loops, meter)? {
            let mut ring = ring_of(body, l)?;
            let mut ends: Option<(P3, P3)> = None;
            while let Some(fin) = next_fin(body, &mut ring, meter)? {
                let q = polygon_point(body, fin)?;
                if q == p {
                    return Ok(None);
                }
                ends = Some(match ends {
                    None => (q, q),
                    Some((first, last)) => {
                        let term = crossing(p, first, last, q)?;
                        count = count.checked_add(term).ok_or(QueryError::Overflow)?;
                        (first, q)
                    }
                });
            }
        }
    }
    Ok(Some(if count == 0 {
        Side::Outside
    } else {
        Side::Inside
    }))
}

/// Which side of shell `b` shell `a` lies on: the side of the first of `a`'s vertices that is not
/// a vertex of `b`, or `None` if `b` holds every vertex of `a`. Two shells that do not cross lie
/// wholly on one side of each other, so one vertex decides.
///
/// # Errors
///
/// [`side_of_shell`]'s.
pub fn shell_side(
    body: &Body,
    a: ShellIx,
    b: ShellIx,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Side>, QueryError> {
    let mut faces = faces_of(body, a)?;
    while let Some(face) = next_face(body, &mut faces, meter)? {
        let mut loops = loops_of(body, face)?;
        while let Some(l) = next_loop(body, &mut loops, meter)? {
            let mut ring = ring_of(body, l)?;
            while let Some(fin) = next_fin(body, &mut ring, meter)? {
                let side = side_of_shell(body, b, polygon_point(body, fin)?, meter)?;
                if side.is_some() {
                    return Ok(side);
                }
            }
        }
    }
    Ok(None)
}

/// Every face of `body`, in arena order.
///
/// # Errors
///
/// The walk's and the meter's errors; a refused reservation as the meter's stop.
pub fn faces(body: &Body, meter: &mut WorkMeter<'_>) -> Result<Vec<FaceRow>, QueryError> {
    let a = body.arenas();
    let names = body.names();
    let mut out: Vec<FaceRow> = reserve(meter, u64::from(a.faces.len()))?;
    for row in 0..a.faces.len() {
        meter.charge(1)?;
        let face = FaceIx::new(row).ok_or(QueryError::NoSuchRow)?;
        let f = a.faces.get(face).ok_or(QueryError::NoSuchRow)?;
        let mut loops = 0_u32;
        let mut walk = loops_of(body, face)?;
        while next_loop(body, &mut walk, meter)?.is_some() {
            loops = loops.saturating_add(1);
        }
        out.push(FaceRow {
            face,
            shell: f.shell,
            surface: f.surface,
            sense: f.sense,
            loops,
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
