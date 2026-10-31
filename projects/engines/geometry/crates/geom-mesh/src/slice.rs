//! Plane slices of closed exact meshes (19.5): layers for 3D printing, and plan cuts.
//!
//! What it does: [`slice`] cuts a mesh by parallel planes in rising order. Each plane gives closed
//! contours, each a ring of [`Cut`]s: a vertex of the mesh on the plane, or a half-edge that crosses
//! it. [`point`] gives a cut's exact point, and nothing is rounded before someone asks for a grid
//! point with [`Point::to_grid`].
//!
//! Guarantees:
//!
//! - **Every contour closes.** A contour follows the mesh's own pairing of half-edges from face to
//!   face, so a closed mesh gives closed rings with no tolerance anywhere.
//! - **A plane cuts just under itself.** A vertex on the plane counts as above it, the one rule
//!   every vertex takes, so an edge crosses or does not with no third case. A box from z = 0 to
//!   z = 1 has a contour at z = 1 and none at z = 0. A contour that closes up to under three points
//!   as the plane comes up to it, such as the tip of a cone, is left out.
//! - **Which side is solid.** Seen from where the planes' normal points, a contour turns
//!   counter-clockwise round solid and clockwise round a cavity.
//! - **The threads do not show.** Contours come in the order of their lowest half-edge, on any
//!   pool.
//!
//! Method: each vertex finds its place among the planes by bisection with exact signs. An edge
//! between places crosses each plane between them. In a face, the crossings on a plane run in
//! pairs along the line where the plane meets the face: most faces have one pair, and the rest are
//! sorted with 11.3's `along` (`projects/engines/geometry/architecture/11-booleans.md`). Pairs join
//! across the mesh's half-edge pairs into rings. Crossing points are three-plane points, the face's,
//! the edge's and the cut's, as 19.5 asks.
//!
//! Cost: O(v log p) signs for v vertices and p planes, then a pass over the half-edges, then for
//! each plane O(c log c) for its c crossings, plane by plane on the pool. The meter is charged one
//! unit per vertex and one per crossing.

#![allow(
    clippy::many_single_char_names,
    reason = "planes, half-edges and the two ends of an edge, as chapter 19 names them"
)]

use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_num::grid::P3;
use geom_num::plane::{Plane, Tpi};
use geom_num::{NumError, Sign};
use geom_poly::order::{Line, Ray, along, turn};
use geom_run::{ChunkPlan, Exec, Phase, RunError, WorkMeter};

use crate::exact::{ExactMesh, Facing, MeshError, MeshFault, MeshKind, PLAN, Parts, Vertex, at};

const PLACES: Phase = Phase::new("slice: vertices");
const LAYERS: Phase = Phase::new("slice: planes");

/// One plane at a time: planes near the middle of a model cost the most.
const ONE_LAYER: ChunkPlan = ChunkPlan::fixed::<1>();

/// A point of a contour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cut {
    /// A vertex of the mesh, on the plane.
    Vertex(u32),
    /// Where this half-edge crosses the plane: the lower of the edge's two in a pair.
    Edge(u32),
}

/// A closed ring on one plane, solid on its left seen from where the plane's normal points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contour {
    cuts: Vec<Cut>,
}

impl Contour {
    /// The ring. The last point joins the first.
    #[must_use]
    pub fn cuts(&self) -> &[Cut] {
        &self.cuts
    }
}

/// A cut's exact place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Point {
    /// A grid point.
    Grid(P3),
    /// Where three planes meet.
    Meet(Tpi),
}

impl Point {
    /// The grid point nearest: exact for a grid point, and the first rounding of any other.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] for a point outside the frame.
    pub fn to_grid(&self) -> Result<P3, NumError> {
        match self {
            Point::Grid(p) => Ok(*p),
            Point::Meet(t) => Ok(t.to_grid()?.value()),
        }
    }
}

/// A vertex among the planes: how many are at or under it, and whether the highest of those holds
/// it.
#[derive(Clone, Copy)]
struct Place {
    over: u32,
    on: bool,
}

/// An edge crossing one plane.
#[derive(Clone, Copy)]
struct Crossing {
    half_edge: u32,
    /// Whether the half-edge runs up through the plane.
    up: bool,
    /// The vertex at the edge's upper end.
    upper: u32,
}

/// `facing`'s plane, turned the way it faces.
fn turned(parts: &Parts, facing: Facing) -> Result<Plane, MeshError> {
    let plane = *at(&parts.planes, facing.row)?;
    if facing.flipped {
        Ok(Plane::new(
            plane.n().map(i128::wrapping_neg),
            plane.d().wrapping_neg(),
        )?)
    } else {
        Ok(plane)
    }
}

/// Which side of a plane a vertex is on.
fn side(plane: &Plane, p: Option<P3>, t: Option<&Tpi>) -> Result<Sign, NumError> {
    match (p, t) {
        (Some(p), _) => plane.side(p),
        (_, Some(t)) => plane.side_of_tpi(t),
        _ => Err(NumError::Degenerate),
    }
}

/// Where `vertex` stands among `planes`, which rise.
fn place(parts: &Parts, vertex: &Vertex, planes: &[Plane]) -> Result<Place, MeshError> {
    let (grid, meet) = match *vertex {
        Vertex::Grid(p) => (Some(p), None),
        Vertex::Planes([a, b, c]) => {
            let plane = |row: u32| at(&parts.planes, row).copied();
            (None, Some(Tpi::new(plane(a)?, plane(b)?, plane(c)?)?))
        }
    };
    let (mut low, mut high) = (0, planes.len());
    let mut on = false;
    while low < high {
        let middle = low.saturating_add(high.saturating_sub(low) / 2);
        let plane = planes.get(middle).ok_or(MeshFault::Shape)?;
        match side(plane, grid, meet.as_ref())? {
            Sign::Negative => high = middle,
            Sign::Zero => {
                on = true;
                low = middle.saturating_add(1);
            }
            Sign::Positive => low = middle.saturating_add(1),
        }
    }
    Ok(Place {
        over: crate::exact::index(low)?,
        on,
    })
}

/// The face of each half-edge.
fn faces_of(parts: &Parts) -> Result<Vec<u32>, MeshFault> {
    let mut out = Vec::with_capacity(parts.he_vert.len());
    for (loops, face) in parts.face_loops.windows(2).zip(0_u32..) {
        let &[first, last] = loops else {
            return Err(MeshFault::Shape);
        };
        let (start, end) = (
            *at(&parts.loop_start, first)?,
            *at(&parts.loop_start, last)?,
        );
        out.resize(
            out.len().saturating_add(end.saturating_sub(start) as usize),
            face,
        );
    }
    Ok(out)
}

/// Every crossing, by plane: the list, and where each plane's part of it starts.
fn crossings(
    parts: &Parts,
    places: &[Place],
    layers: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<Crossing>, Vec<usize>), MeshError> {
    let over = |v: u32| at(places, v).map(|p| p.over);
    // Each half-edge with its vertices, from each loop in turn.
    let mut edges: Vec<(u32, u32, u32)> = Vec::new();
    for ends in parts.loop_start.windows(2) {
        let &[start, end] = ends else {
            return Err(MeshFault::Shape.into());
        };
        for h in start..end {
            let next = if h.saturating_add(1) == end {
                start
            } else {
                h.saturating_add(1)
            };
            let (a, b) = (*at(&parts.he_vert, h)?, *at(&parts.he_vert, next)?);
            if over(a)? != over(b)? {
                edges.push((h, a, b));
            }
        }
    }
    let mut first = alloc::vec![0_usize; layers.saturating_add(1)];
    let mut total = 0_usize;
    for &(_, a, b) in &edges {
        let (p, q) = (over(a)?, over(b)?);
        for layer in p.min(q)..p.max(q) {
            let slot = first
                .get_mut((layer as usize).saturating_add(1))
                .ok_or(MeshFault::Shape)?;
            *slot = slot.saturating_add(1);
            total = total.saturating_add(1);
        }
        meter.charge(1)?;
    }
    meter.charge(u64::try_from(total).unwrap_or(u64::MAX))?;
    let mut sum = 0_usize;
    for slot in &mut first {
        sum = sum.saturating_add(*slot);
        *slot = sum;
    }
    let mut fill = first.clone();
    let blank = Crossing {
        half_edge: 0,
        up: false,
        upper: 0,
    };
    let mut out = alloc::vec![blank; total];
    for &(half_edge, a, b) in &edges {
        let (p, q) = (over(a)?, over(b)?);
        let (up, upper) = if p < q { (true, b) } else { (false, a) };
        for layer in p.min(q)..p.max(q) {
            let slot = fill.get_mut(layer as usize).ok_or(MeshFault::Shape)?;
            *out.get_mut(*slot).ok_or(MeshFault::Shape)? = Crossing {
                half_edge,
                up,
                upper,
            };
            *slot = slot.saturating_add(1);
        }
    }
    Ok((out, first))
}

/// The order of two crossings of a face along the line where `outward` meets the plane.
fn in_line(
    parts: &Parts,
    line: &Line,
    outward: &Plane,
    x: &Crossing,
    y: &Crossing,
) -> Result<Ordering, MeshError> {
    let edge = |c: &Crossing| turned(parts, *at(&parts.he_plane, c.half_edge)?);
    let order = along(line, &edge(x)?, &edge(y)?)?;
    if order != Ordering::Equal {
        return Ok(order);
    }
    // One vertex on the plane, the edges leaving it downward: just under the plane, the one further
    // clockwise is further along the line. A half-edge runs against its plane's ray.
    let ray = |c: &Crossing| -> Result<Ray, MeshError> {
        Ok(Ray {
            with: edge(c)?,
            reversed: !c.up,
        })
    };
    Ok(match turn(outward, &ray(x)?, &ray(y)?)? {
        Sign::Positive => Ordering::Less,
        Sign::Negative => Ordering::Greater,
        Sign::Zero => x.half_edge.cmp(&y.half_edge),
    })
}

/// The pieces of `face` on `plane` from all its crossings, each from where the contour comes into
/// the face to where it leaves.
fn paired(
    parts: &Parts,
    plane: &Plane,
    face: u32,
    group: &[Crossing],
    pieces: &mut Vec<(Crossing, Crossing)>,
) -> Result<(), MeshError> {
    let outward = turned(parts, *at(&parts.face_plane, face)?)?;
    let line = Line::new(*plane, outward)?;
    let mut order: Vec<Crossing> = group.to_vec();
    let mut failed = None;
    order.sort_by(|x, y| {
        in_line(parts, &line, &outward, x, y).unwrap_or_else(|e| {
            failed = Some(e);
            Ordering::Equal
        })
    });
    if let Some(e) = failed {
        return Err(e);
    }
    let mut walk = order.iter();
    while let (Some(&coming), Some(&going)) = (walk.next(), walk.next()) {
        if coming.up || !going.up {
            return Err(MeshError::Defect);
        }
        pieces.push((coming, going));
    }
    Ok(())
}

/// The cut where a contour leaves a face by `c`, on plane number `layer`.
fn cut_of(parts: &Parts, places: &[Place], c: &Crossing, layer: u32) -> Result<Cut, MeshFault> {
    let upper = at(places, c.upper)?;
    if upper.on && upper.over == layer.saturating_add(1) {
        return Ok(Cut::Vertex(c.upper));
    }
    let pair = *at(&parts.he_pair, c.half_edge)?;
    Ok(Cut::Edge(c.half_edge.min(pair)))
}

/// The contours of one plane.
fn contours(
    parts: &Parts,
    places: &[Place],
    face_of: &[u32],
    list: &[Crossing],
    plane: &Plane,
    layer: u32,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Contour>, MeshError> {
    let mut pieces: Vec<(Crossing, Crossing)> = Vec::with_capacity(list.len() / 2);
    let mut rest = list;
    while let Some((head, _)) = rest.split_first() {
        let face = *at(face_of, head.half_edge)?;
        let same = rest
            .iter()
            .take_while(|c| face_of.get(c.half_edge as usize) == Some(&face))
            .count();
        let (group, tail) = rest.split_at(same);
        rest = tail;
        match group {
            &[a, b] if a.up != b.up => pieces.push(if a.up { (b, a) } else { (a, b) }),
            _ => paired(parts, plane, face, group, &mut pieces)?,
        }
    }
    pieces.sort_unstable_by_key(|(coming, _)| coming.half_edge);
    let mut done = alloc::vec![false; pieces.len()];
    let mut out = Vec::new();
    for start in 0..pieces.len() {
        if done.get(start) != Some(&false) {
            continue;
        }
        let mut cuts = Vec::new();
        let mut here = start;
        loop {
            meter.charge(1)?;
            *done.get_mut(here).ok_or(MeshFault::Shape)? = true;
            let (_, going) = pieces.get(here).ok_or(MeshFault::Shape)?;
            cuts.push(cut_of(parts, places, going, layer)?);
            let next = *at(&parts.he_pair, going.half_edge)?;
            here = pieces
                .binary_search_by_key(&next, |(coming, _)| coming.half_edge)
                .map_err(|_| MeshError::Defect)?;
            if here == start {
                break;
            }
        }
        cuts.dedup();
        if cuts.len() > 1 && cuts.first() == cuts.last() {
            cuts.pop();
        }
        if cuts.len() >= 3 {
            out.push(Contour { cuts });
        }
    }
    Ok(out)
}

/// The contours of `mesh` on each of `planes`, which must be parallel and rise along their normal.
///
/// # Errors
///
/// [`MeshError::NotClosed`] for a mesh with boundary edges, [`MeshError::SlicesOutOfOrder`] for
/// planes not parallel or out of order, and a stopped meter.
pub fn slice(
    mesh: &ExactMesh,
    planes: &[Plane],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Vec<Contour>>, MeshError> {
    if mesh.kind() == MeshKind::Open {
        return Err(MeshError::NotClosed);
    }
    let rising = |pair: &[Plane]| match pair {
        [a, b] => a.n() == b.n() && a.d() > b.d(),
        _ => true,
    };
    if !planes.windows(2).all(rising) {
        return Err(MeshError::SlicesOutOfOrder);
    }
    let parts = mesh.parts();
    let found: Result<Vec<Place>, RunError<MeshError>> =
        exec.try_map(meter, PLACES, PLAN, &parts.verts, |_, v, m| {
            m.charge(1)?;
            Ok(place(parts, v, planes)?)
        });
    let places = found.map_err(MeshError::from)?;
    let face_of = faces_of(parts)?;
    let (list, first) = crossings(parts, &places, planes.len(), meter)?;
    let numbered: Vec<u32> = (0..crate::exact::index(planes.len())?).collect();
    let out: Result<Vec<Vec<Contour>>, RunError<MeshError>> =
        exec.try_map(meter, LAYERS, ONE_LAYER, &numbered, |_, &layer, m| {
            let row = layer as usize;
            let start = *first.get(row).ok_or(MeshFault::Shape)?;
            let end = *first.get(row.saturating_add(1)).ok_or(MeshFault::Shape)?;
            let part = list.get(start..end).ok_or(MeshFault::Shape)?;
            let plane = planes.get(row).ok_or(MeshFault::Shape)?;
            Ok(contours(parts, &places, &face_of, part, plane, layer, m)?)
        });
    out.map_err(MeshError::from)
}

/// The exact place of `cut` on `plane`.
///
/// # Errors
///
/// [`MeshFault::Shape`] for a cut that is not of this mesh.
pub fn point(mesh: &ExactMesh, plane: &Plane, cut: Cut) -> Result<Point, MeshError> {
    let parts = mesh.parts();
    match cut {
        Cut::Vertex(v) => match *at(&parts.verts, v)? {
            Vertex::Grid(p) => Ok(Point::Grid(p)),
            Vertex::Planes([a, b, c]) => {
                let row = |r: u32| at(&parts.planes, r).copied();
                Ok(Point::Meet(Tpi::new(row(a)?, row(b)?, row(c)?)?))
            }
        },
        Cut::Edge(h) => {
            let looped = parts.loop_start.partition_point(|s| *s <= h);
            let looped = crate::exact::index(looped.saturating_sub(1))?;
            let face = parts.face_loops.partition_point(|l| *l <= looped);
            let face = crate::exact::index(face.saturating_sub(1))?;
            let facing = *at(&parts.face_plane, face)?;
            let through = *at(&parts.he_plane, h)?;
            Ok(Point::Meet(Tpi::new(
                *at(&parts.planes, facing.row)?,
                *at(&parts.planes, through.row)?,
                *plane,
            )?))
        }
    }
}
