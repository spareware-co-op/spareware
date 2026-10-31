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
//!   `geom-mesh`'s, and it recomputes every point it compares.
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

use std::collections::BTreeMap;

use dashu_int::IBig;
use dashu_ratio::RBig;
use geom_mesh::exact::{Facing, MeshKind, NONE, Parts, Vertex};
use geom_num::P3;

/// A point or a vector.
type V3 = [RBig; 3];

/// One rule of 19.2 broken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finding {
    /// The arrays do not fit together.
    Shape,
    /// A vertex's three planes do not meet in one point.
    NoPoint(u32),
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
}

/// A number as a rational.
fn q(n: i128) -> RBig {
    RBig::from(IBig::from(n))
}

/// A grid point as a rational point.
fn point(p: P3) -> V3 {
    [p.x, p.y, p.z].map(|c| q(i128::from(c.get())))
}

/// `a × b`.
fn cross(a: &V3, b: &V3) -> V3 {
    [
        &a[1] * &b[2] - &a[2] * &b[1],
        &a[2] * &b[0] - &a[0] * &b[2],
        &a[0] * &b[1] - &a[1] * &b[0],
    ]
}

/// `a · b`.
fn dot(a: &V3, b: &V3) -> RBig {
    &a[0] * &b[0] + &a[1] * &b[1] + &a[2] * &b[2]
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
            Some([0, 1, 2].map(|k| -(&da * &across[k] + &db * &ca[k] + &dc * &ab[k]) / &det))
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
    let rising = |list: &[u32]| list.windows(2).all(|w| w[0] <= w[1]);
    parts.face_loops.len() == faces + 1
        && rising(&parts.face_loops)
        && rising(&parts.loop_start)
        && parts.face_loops.last().map(|v| *v as usize) == parts.loop_start.len().checked_sub(1)
        && parts.loop_start.last().map(|v| *v as usize) == Some(edges)
        && parts.he_plane.len() == edges
        && parts.he_pair.len() == edges
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

/// Twice the area vector of a face's rings, its holes running the other way.
fn area(parts: &Parts, points: &[V3], f: usize) -> V3 {
    let mut sum = [RBig::ZERO, RBig::ZERO, RBig::ZERO];
    for l in loops(parts, f) {
        for h in ring(parts, l) {
            let (a, b) = (
                &points[parts.he_vert[h] as usize],
                &points[parts.he_vert[next_in(parts, l, h)] as usize],
            );
            let c = cross(a, b);
            sum = [0, 1, 2].map(|k| &sum[k] + &c[k]);
        }
    }
    sum
}

/// The rules each face keeps.
fn faces(parts: &Parts, points: &[V3], out: &mut Vec<Finding>) {
    for f in 0..parts.face_plane.len() {
        let face = u32::try_from(f).unwrap_or(u32::MAX);
        let Some((n, d)) = facing(parts, parts.face_plane[f]) else {
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
                let (from, to) = (parts.he_vert[h], parts.he_vert[next_in(parts, l, h)]);
                if from == to {
                    out.push(Finding::ZeroEdge(edge));
                }
                let (a, b) = (&points[from as usize], &points[to as usize]);
                if dot(&n, a) + &d != RBig::ZERO {
                    out.push(Finding::OffFace(edge));
                }
                let on_edge = plane(parts, parts.he_plane[h].row).is_some_and(|(m, e)| {
                    dot(&m, a) + &e == RBig::ZERO && dot(&m, b) + &e == RBig::ZERO
                });
                if !on_edge {
                    out.push(Finding::OffEdgePlane(edge));
                }
            }
        }
        let along = dot(&area(parts, points, f), &n);
        if along == RBig::ZERO {
            out.push(Finding::NoArea(face));
        } else if along < RBig::ZERO {
            out.push(Finding::WrongWay(face));
        }
    }
}

/// The rules each edge keeps: used equally each way, paired with its opposite, the mesh's kind.
fn edges(parts: &Parts, out: &mut Vec<Finding>) {
    let mut uses: BTreeMap<(u32, u32), (u32, u32, u32)> = BTreeMap::new();
    let mut ends: Vec<(u32, u32)> = vec![(0, 0); parts.he_vert.len()];
    for l in 0..parts.loop_start.len().saturating_sub(1) {
        for h in ring(parts, l) {
            let (a, b) = (parts.he_vert[h], parts.he_vert[next_in(parts, l, h)]);
            ends[h] = (a, b);
            let slot = uses.entry((a.min(b), a.max(b))).or_insert((0, 0, 0));
            if a < b {
                slot.0 += 1;
            } else {
                slot.1 += 1;
            }
            slot.2 = u32::try_from(h).unwrap_or(u32::MAX);
        }
    }
    let mut boundary = false;
    let mut radial = false;
    for (ahead, back, h) in uses.values() {
        match (*ahead, *back) {
            (1, 1) => {}
            (1, 0) | (0, 1) => boundary = true,
            (a, b) if a == b => radial = true,
            _ => out.push(Finding::Uneven(*h)),
        }
    }
    let right = match parts.kind {
        MeshKind::Closed => !boundary && !radial,
        MeshKind::Open => !radial,
        MeshKind::General => radial && !boundary,
    };
    if !right {
        out.push(Finding::WrongKind);
    }
    for (h, &pair) in parts.he_pair.iter().enumerate() {
        let row = u32::try_from(h).unwrap_or(u32::MAX);
        if pair == NONE {
            let alone = uses.get(&(ends[h].0.min(ends[h].1), ends[h].0.max(ends[h].1)));
            if !alone.is_some_and(|u| u.0 + u.1 == 1) {
                out.push(Finding::BadPair(row));
            }
        } else if ends.get(pair as usize) != Some(&(ends[h].1, ends[h].0)) {
            out.push(Finding::BadPair(row));
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
            Some(p) => points.push(p),
            None => out.push(Finding::NoPoint(u32::try_from(v).unwrap_or(u32::MAX))),
        }
    }
    if !out.is_empty() {
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
            .map(|h| parts.he_vert[h])
            .collect();
        if let Some(first) = rings.insert(corners, row) {
            out.push(Finding::Repeated(first, row));
        }
    }
    faces(parts, &points, &mut out);
    edges(parts, &mut out);
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
