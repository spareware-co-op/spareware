//! The second checker for exact meshes (chapter 28.4; piece G135): 19.2's rules, written again from
//! their definitions in exact rationals, sharing no code with `geom-mesh`'s checker.
//!
//! What it does: [`check`] reads a mesh's arrays into rationals, each vertex a point (three planes
//! solved by Cramer's rule), and tests each rule the direct way. [`six_volume`] sums a mesh's volume
//! as an exact rational.
//!
//! Guarantees:
//!
//! - **Exact.** Every value is a rational; nothing rounds and no width can overflow.
//! - **Independent.** It reads `Parts` and nothing of the checker: no predicate, no key, no sort of
//!   `geom-mesh`'s, and it recomputes every point it compares. Around an edge of more than two
//!   half-edges it sorts the faces by the signs of rational cross products.
//! - **Off the grid as on it.** A face with a corner off the grid has its turn and its area tested
//!   like any other, and an edge its facing, which the first checker leaves to the rest of the mesh.
//! - **Says what it finds.** A list of [`Finding`]s, so a test can hold it to the first checker's
//!   verdict on every body, valid or broken.
//!
//! Method: chapter 19.2's rules (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`);
//! Cramer's rule for three planes; Newell's vector area of a polygon (Goldman, Graphics Gems II,
//! 1991, doi:10.1016/B978-0-08-050754-5.50043-8); the divergence theorem for its volume.
//!
//! Cost: rationals grow with the depth of the sums, and a mesh costs a sort of its half-edges. This
//! checker accepts that, because it runs on tests only. No recursion.

// The lint is for fixed-width arithmetic, which can overflow. Every operator here is on dashu's
// arbitrary-precision rationals, which cannot; every division is by a value shown nonzero.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "exact rational arithmetic, which cannot overflow; divisors are shown nonzero"
)]

use core::cmp::Ordering;
use std::collections::BTreeMap;

use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;
use geom_mesh::exact::{Facing, MeshKind, NONE, Parts, Vertex};
use geom_num::{Exactness, P3};

/// A point or a vector.
type V3 = [RBig; 3];

/// One rule of 19.2 broken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finding {
    /// The arrays do not fit together.
    Shape,
    /// A vertex's three planes do not meet in one point.
    NoPoint(u32),
    /// A meeting of planes at a grid point, which belongs in that form.
    OnTheGrid(u32),
    /// Two vertices at one place.
    OnePoint(u32, u32),
    /// A loop of fewer than three half-edges.
    ShortLoop(u32),
    /// A half-edge from a vertex to itself.
    ZeroEdge(u32),
    /// A corner off its face's plane.
    OffFace(u32),
    /// An end of a half-edge off its edge plane.
    OffEdgePlane(u32),
    /// An edge plane not facing into its face.
    EdgeFacing(u32),
    /// A face with no area.
    NoArea(u32),
    /// A face turning the other way from its plane.
    WrongWay(u32),
    /// A half-edge's pair not running the other way along its edge.
    BadPair(u32),
    /// An edge not used equally often each way.
    Uneven(u32),
    /// A mesh not the kind it says.
    WrongKind,
    /// Two faces with the same corners in the same order.
    Repeated(u32, u32),
    /// A face further from its source than the mesh's class says.
    Deviation(u32),
}

/// A number as a rational.
fn q(n: i128) -> RBig {
    RBig::from(IBig::from(n))
}

/// A grid point as a rational point.
fn point(p: P3) -> V3 {
    [p.x, p.y, p.z].map(|c| q(i128::from(c.get())))
}

/// `f` of each pair of coordinates.
fn zipped(a: &V3, b: &V3, f: impl Fn(&RBig, &RBig) -> RBig) -> V3 {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    [f(ax, bx), f(ay, by), f(az, bz)]
}

/// `a − b`.
fn minus(a: &V3, b: &V3) -> V3 {
    zipped(a, b, |x, y| x - y)
}

/// `a + b`.
fn plus(a: &V3, b: &V3) -> V3 {
    zipped(a, b, |x, y| x + y)
}

/// `a × b`.
fn cross(a: &V3, b: &V3) -> V3 {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}

/// `a · b`.
fn dot(a: &V3, b: &V3) -> RBig {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    ax * bx + ay * by + az * bz
}

/// The pool's plane at `row`, as a normal and an offset.
fn plane(parts: &Parts, row: u32) -> Option<(V3, RBig)> {
    let p = parts.planes.get(row as usize)?;
    Some((p.n().map(q), q(p.d())))
}

/// `face`'s plane, as a normal pointing the way the face looks, and an offset.
fn facing(parts: &Parts, face: Facing) -> Option<(V3, RBig)> {
    let (n, d) = plane(parts, face.row)?;
    Some(if face.flipped {
        (n.map(|c| -c), -d)
    } else {
        (n, d)
    })
}

/// Where a vertex lies, by Cramer's rule; the determinant is shown nonzero before it divides.
fn place(parts: &Parts, v: &Vertex) -> Option<V3> {
    match v {
        Vertex::Grid(p) => Some(point(*p)),
        Vertex::Planes([a, b, c]) => {
            let ((na, da), (nb, db), (nc, dc)) =
                (plane(parts, *a)?, plane(parts, *b)?, plane(parts, *c)?);
            let across = cross(&nb, &nc);
            let det = dot(&na, &across);
            if det == RBig::ZERO {
                return None;
            }
            let (ca, ab) = (cross(&nc, &na), cross(&na, &nb));
            let sum = plus(
                &across.map(|c| &da * c),
                &plus(&ca.map(|c| &db * c), &ab.map(|c| &dc * c)),
            );
            Some(sum.map(|c| -c / &det))
        }
    }
}

/// The half-edge rows of loop `l`.
fn ring(parts: &Parts, l: usize) -> std::ops::Range<usize> {
    let at = |k: usize| parts.loop_start.get(k).map_or(0, |v| *v as usize);
    at(l)..at(l + 1)
}

/// The loop rows of face `f`.
fn loops(parts: &Parts, f: usize) -> std::ops::Range<usize> {
    let at = |k: usize| parts.face_loops.get(k).map_or(0, |v| *v as usize);
    at(f)..at(f + 1)
}

/// Whether the arrays fit together.
fn fits(parts: &Parts) -> bool {
    let faces = parts.face_plane.len();
    let edges = parts.he_vert.len();
    let rising =
        |list: &[u32]| list.first() == Some(&0) && list.windows(2).all(|w| w.first() <= w.last());
    parts.face_loops.len() == faces + 1
        && rising(&parts.face_loops)
        && rising(&parts.loop_start)
        && parts.face_loops.last().map(|v| *v as usize) == parts.loop_start.len().checked_sub(1)
        && parts.loop_start.last().map(|v| *v as usize) == Some(edges)
        && parts.he_plane.len() == edges
        && parts.he_pair.len() == edges
        && parts.he_prop.len() == edges
        && parts.face_dev.len() == faces
        && parts
            .he_vert
            .iter()
            .all(|v| (*v as usize) < parts.verts.len())
}

/// The half-edge after `h` in its loop `l`.
fn next_in(parts: &Parts, l: usize, h: usize) -> usize {
    let span = ring(parts, l);
    if h + 1 == span.end { span.start } else { h + 1 }
}

/// The vertices half-edge `h` of loop `l` runs between.
fn ends(parts: &Parts, l: usize, h: usize) -> Option<(u32, u32)> {
    let at = |row: usize| parts.he_vert.get(row).copied();
    Some((at(h)?, at(next_in(parts, l, h))?))
}

/// The places half-edge `h` of loop `l` runs between.
fn places<'p>(parts: &Parts, points: &'p [V3], l: usize, h: usize) -> Option<(&'p V3, &'p V3)> {
    let (from, to) = ends(parts, l, h)?;
    Some((points.get(from as usize)?, points.get(to as usize)?))
}

/// Twice the area vector of a face's rings, its holes running the other way.
fn area(parts: &Parts, points: &[V3], f: usize) -> Option<V3> {
    let mut sum = [RBig::ZERO, RBig::ZERO, RBig::ZERO];
    for l in loops(parts, f) {
        for h in ring(parts, l) {
            let (a, b) = places(parts, points, l, h)?;
            sum = plus(&sum, &cross(a, b));
        }
    }
    Some(sum)
}

/// The rules each face keeps.
fn faces(parts: &Parts, points: &[V3], out: &mut Vec<Finding>) {
    for f in 0..parts.face_plane.len() {
        let face = u32::try_from(f).unwrap_or(u32::MAX);
        let Some((n, d)) = parts.face_plane.get(f).and_then(|p| facing(parts, *p)) else {
            out.push(Finding::Shape);
            continue;
        };
        for l in loops(parts, f) {
            let row = u32::try_from(l).unwrap_or(u32::MAX);
            if ring(parts, l).len() < 3 {
                out.push(Finding::ShortLoop(row));
                continue;
            }
            for h in ring(parts, l) {
                let edge = u32::try_from(h).unwrap_or(u32::MAX);
                let (Some((from, to)), Some((a, b))) =
                    (ends(parts, l, h), places(parts, points, l, h))
                else {
                    out.push(Finding::Shape);
                    continue;
                };
                if from == to {
                    out.push(Finding::ZeroEdge(edge));
                }
                if dot(&n, a) + &d != RBig::ZERO {
                    out.push(Finding::OffFace(edge));
                }
                let Some((m, e)) = parts.he_plane.get(h).and_then(|p| facing(parts, *p)) else {
                    out.push(Finding::Shape);
                    continue;
                };
                if dot(&m, a) + &e != RBig::ZERO || dot(&m, b) + &e != RBig::ZERO {
                    out.push(Finding::OffEdgePlane(edge));
                }
                // The face lies to the left of its edge, seen from outside: along n × (b − a).
                if dot(&m, &cross(&n, &minus(b, a))) <= RBig::ZERO {
                    out.push(Finding::EdgeFacing(edge));
                }
            }
        }
        let Some(vector) = area(parts, points, f) else {
            out.push(Finding::Shape);
            continue;
        };
        let along = dot(&vector, &n);
        if along == RBig::ZERO {
            out.push(Finding::NoArea(face));
        } else if along < RBig::ZERO {
            out.push(Finding::WrongWay(face));
        }
    }
}

/// One half-edge's flap about its edge: the way its face runs from the edge.
struct Flap {
    half_edge: u32,
    /// Whether it runs from the edge's higher vertex to its lower.
    against: bool,
    run: V3,
}

/// Where `run` lies from `first`, turning about `axis`: 0 on it, 1 within a half turn, 2 opposite,
/// 3 beyond.
fn rank(first: &V3, run: &V3, axis: &V3) -> u8 {
    let side = dot(&cross(first, run), axis);
    if side > RBig::ZERO {
        1
    } else if side < RBig::ZERO {
        3
    } else if dot(first, run) > RBig::ZERO {
        0
    } else {
        2
    }
}

/// The half-edges of an edge from `low` to `high`, each with its flap, in the order they turn about
/// the line from `low` to `high`; flaps in one half-plane by row.
fn around(
    parts: &Parts,
    points: &[V3],
    owner: &[u32],
    group: &[u32],
    (low, high): (u32, u32),
) -> Option<Vec<Flap>> {
    let axis = minus(points.get(high as usize)?, points.get(low as usize)?);
    let mut flaps = Vec::new();
    for &h in group {
        let row = h as usize;
        let (n, _) = facing(parts, *parts.face_plane.get(*owner.get(row)? as usize)?)?;
        let against = parts.he_vert.get(row) == Some(&high);
        let way = if against {
            axis.clone().map(|c| -c)
        } else {
            axis.clone()
        };
        flaps.push(Flap {
            half_edge: h,
            against,
            run: cross(&n, &way),
        });
    }
    let first = flaps.first()?.run.clone();
    flaps.sort_by(|p, q| {
        rank(&first, &p.run, &axis)
            .cmp(&rank(&first, &q.run, &axis))
            .then_with(|| {
                RBig::ZERO
                    .partial_cmp(&dot(&cross(&p.run, &q.run), &axis))
                    .unwrap_or(Ordering::Equal)
            })
            .then(p.half_edge.cmp(&q.half_edge))
    });
    Some(flaps)
}

/// An edge of more than two half-edges: each pair bounds one cell, so a half-edge against the line
/// meets its pair next, turning about the line from its face through what lies behind it.
fn radial(
    parts: &Parts,
    points: &[V3],
    owner: &[u32],
    group: &[u32],
    edge: (u32, u32),
    out: &mut Vec<Finding>,
) {
    let Some(flaps) = around(parts, points, owner, group, edge) else {
        out.push(Finding::Shape);
        return;
    };
    for (flap, next) in flaps.iter().zip(flaps.iter().cycle().skip(1)) {
        let pair = |h: u32| parts.he_pair.get(h as usize).copied();
        let paired = pair(flap.half_edge) == Some(next.half_edge)
            && pair(next.half_edge) == Some(flap.half_edge);
        if flap.against && (next.against || !paired) {
            out.push(Finding::BadPair(flap.half_edge));
        }
    }
}

/// The rules each edge keeps: used equally each way, paired, and the mesh the kind it says.
fn edges(parts: &Parts, points: &[V3], out: &mut Vec<Finding>) {
    let mut uses: BTreeMap<(u32, u32), Vec<u32>> = BTreeMap::new();
    let mut owner = vec![0; parts.he_vert.len()];
    for f in 0..parts.face_plane.len() {
        for l in loops(parts, f) {
            for h in ring(parts, l) {
                let Some((a, b)) = ends(parts, l, h) else {
                    continue;
                };
                if let Some(slot) = owner.get_mut(h) {
                    *slot = u32::try_from(f).unwrap_or(u32::MAX);
                }
                let row = u32::try_from(h).unwrap_or(u32::MAX);
                uses.entry((a.min(b), a.max(b))).or_default().push(row);
            }
        }
    }
    let (mut boundary, mut general) = (false, false);
    for (&edge, group) in &uses {
        let of = |h: u32| parts.he_vert.get(h as usize).copied();
        let ahead = group.iter().filter(|h| of(**h) == Some(edge.0)).count();
        let pair = |h: u32| parts.he_pair.get(h as usize).copied();
        match group.as_slice() {
            [one] => {
                boundary = true;
                if pair(*one) != Some(NONE) {
                    out.push(Finding::BadPair(*one));
                }
            }
            [x, y] if ahead == 1 => {
                if pair(*x) != Some(*y) || pair(*y) != Some(*x) {
                    out.push(Finding::BadPair(*x));
                }
            }
            [first, ..] if ahead * 2 != group.len() => out.push(Finding::Uneven(*first)),
            _ => {
                general = true;
                radial(parts, points, &owner, group, edge, out);
            }
        }
    }
    let kind = if general {
        MeshKind::General
    } else if boundary {
        MeshKind::Open
    } else {
        MeshKind::Closed
    };
    if parts.kind != kind {
        out.push(Finding::WrongKind);
    }
}

/// No face further from its source than the class says.
fn deviations(parts: &Parts, out: &mut Vec<Finding>) {
    let limit = match parts.class {
        Exactness::Exact => 0,
        Exactness::Certified { bound, .. } => bound.get(),
        Exactness::Approximate { deviation, .. } => deviation.get(),
    };
    for (face, dev) in parts.face_dev.iter().enumerate() {
        if dev.get() > limit {
            out.push(Finding::Deviation(u32::try_from(face).unwrap_or(u32::MAX)));
        }
    }
}

/// Every rule `parts` break, as far as this checker reads them.
#[must_use]
pub fn check(parts: &Parts) -> Vec<Finding> {
    if !fits(parts) {
        return vec![Finding::Shape];
    }
    let mut out = Vec::new();
    let mut points = Vec::new();
    for (v, vertex) in parts.verts.iter().enumerate() {
        match place(parts, vertex) {
            Some(p) => {
                let whole = p.iter().all(|c| *c.denominator() == UBig::ONE);
                if whole && matches!(vertex, Vertex::Planes(_)) {
                    out.push(Finding::OnTheGrid(u32::try_from(v).unwrap_or(u32::MAX)));
                }
                points.push(p);
            }
            None => out.push(Finding::NoPoint(u32::try_from(v).unwrap_or(u32::MAX))),
        }
    }
    if points.len() != parts.verts.len() {
        return out;
    }
    let mut seen: BTreeMap<&V3, u32> = BTreeMap::new();
    for (v, p) in points.iter().enumerate() {
        let row = u32::try_from(v).unwrap_or(u32::MAX);
        if let Some(first) = seen.insert(p, row) {
            out.push(Finding::OnePoint(first, row));
        }
    }
    let mut rings: BTreeMap<Vec<u32>, u32> = BTreeMap::new();
    for f in 0..parts.face_plane.len() {
        let row = u32::try_from(f).unwrap_or(u32::MAX);
        let corners: Vec<u32> = loops(parts, f)
            .flat_map(|l| ring(parts, l))
            .filter_map(|h| parts.he_vert.get(h).copied())
            .collect();
        if let Some(first) = rings.insert(corners, row) {
            out.push(Finding::Repeated(first, row));
        }
    }
    faces(parts, &points, &mut out);
    edges(parts, &points, &mut out);
    deviations(parts, &mut out);
    out
}

/// Six times the volume of a mesh, exactly: over each loop, a fan from its first corner.
#[must_use]
pub fn six_volume(parts: &Parts) -> Option<RBig> {
    let points: Vec<V3> = parts
        .verts
        .iter()
        .map(|v| place(parts, v))
        .collect::<Option<_>>()?;
    let mut sum = RBig::ZERO;
    for l in 0..parts.loop_start.len().saturating_sub(1) {
        let span: Vec<usize> = ring(parts, l).collect();
        let (first, rest) = span.split_first()?;
        let apex = points.get(*parts.he_vert.get(*first)? as usize)?;
        for pair in rest.windows(2) {
            let a = points.get(*parts.he_vert.get(*pair.first()?)? as usize)?;
            let b = points.get(*parts.he_vert.get(*pair.get(1)?)? as usize)?;
            sum += dot(apex, &cross(a, b));
        }
    }
    Some(sum)
}
