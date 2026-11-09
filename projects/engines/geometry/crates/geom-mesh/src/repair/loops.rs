//! What healing's steps share: faces as loops of vertex numbers, their edges, and exact integer
//! geometry on grid points.
//!
//! What it does: a face is a loop of vertex numbers, and the vertices are grid points numbered in
//! key order, so a number's order is its key's. [`uses`] lists every edge use, sorted by edge;
//! [`split`] cuts a loop at a repeated vertex into simple loops; [`fan`] splits a loop into
//! triangles from its first corner; the rest is integer geometry on grid points.
//!
//! Guarantees:
//!
//! - **Exact.** Coordinates are within ±(2^40 − 1), so a difference is below 2^41, a product of two
//!   below 2^82 and a sum of three such below 2^84: every value here fits `i128`, and the saturating
//!   steps never saturate.
//! - **No order of its own.** Every list comes out sorted by vertex numbers, so its order is the
//!   keys' order.
//!
//! Method: integer vector algebra on grid points, 19.2's grid vertices
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`); a fan from one corner,
//! which has the loop's winding number everywhere.
//!
//! Cost: linear in a loop's corners, but for [`uses`]' sort.

use alloc::vec::Vec;

use geom_num::grid::P3;

use crate::exact::MeshError;

/// A face's corners, as vertex numbers in order round it.
pub(crate) type Loop = Vec<u32>;

/// A face as the steps carry it: its corners, the input face it came from, and whether a corner
/// of that face moved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Face {
    pub corners: Loop,
    pub input: u32,
    pub moved: bool,
}

/// One use of an edge by a face: its ends, the lower first, the face, where in the loop the edge
/// starts, and whether the face runs from the lower end to the higher.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Use {
    pub low: u32,
    pub high: u32,
    pub face: u32,
    pub at: u32,
    pub rising: bool,
}

/// Every edge use of `faces`, sorted by edge, then face and place.
pub(crate) fn uses(faces: &[Face]) -> Result<Vec<Use>, MeshError> {
    let mut out = Vec::new();
    for (face, looped) in (0_u32..).zip(faces.iter().map(|f| &f.corners)) {
        for (at, (&a, &b)) in (0_u32..).zip(looped.iter().zip(looped.iter().cycle().skip(1))) {
            out.try_reserve(1).map_err(|_| MeshError::OutOfMemory)?;
            out.push(Use {
                low: a.min(b),
                high: a.max(b),
                face,
                at,
                rising: a < b,
            });
        }
    }
    out.sort_unstable();
    Ok(out)
}

/// The uses of one edge at a time, from a list sorted by edge.
pub(crate) fn by_edge(uses: &[Use]) -> impl Iterator<Item = &[Use]> {
    uses.chunk_by(|a, b| (a.low, a.high) == (b.low, b.high))
}

/// `looped` without a corner repeating the one before it, cut at each vertex it passes twice into
/// simple loops: a pinched loop is two loops meeting at a point. Loops of fewer than three corners
/// come out too, for the caller to drop.
pub(crate) fn split(looped: &[u32]) -> Vec<Loop> {
    let mut stack: Loop = Vec::with_capacity(looped.len());
    let mut out = Vec::new();
    for &v in looped {
        if stack.last() == Some(&v) {
            continue;
        }
        if let Some(at) = stack.iter().position(|&w| w == v) {
            out.push(stack.split_off(at));
        }
        stack.push(v);
    }
    out.push(stack);
    out
}

/// `looped` turned to start at its lowest vertex, the same way round.
pub(crate) fn lowest_first(looped: &mut [u32]) {
    if let Some(at) = looped
        .iter()
        .enumerate()
        .min_by_key(|&(_, v)| *v)
        .map(|(at, _)| at)
    {
        looped.rotate_left(at);
    }
}

/// `looped` the other way round, still starting at its first corner.
pub(crate) fn reversed(looped: &[u32]) -> Loop {
    let mut out = Loop::with_capacity(looped.len());
    if let Some((&first, rest)) = looped.split_first() {
        out.push(first);
        out.extend(rest.iter().rev());
    }
    out
}

/// The triangles of a fan from `looped`'s first corner, those with no area left out.
pub(crate) fn fan(looped: &[u32], points: &[P3]) -> Result<Vec<Loop>, MeshError> {
    let mut out = Vec::new();
    let Some((&first, rest)) = looped.split_first() else {
        return Ok(out);
    };
    for pair in rest.windows(2) {
        let &[b, c] = pair else { continue };
        if !collinear(point(points, first)?, point(points, b)?, point(points, c)?) {
            out.push(alloc::vec![first, b, c]);
        }
    }
    Ok(out)
}

/// Vertex `v`'s point.
pub(crate) fn point(points: &[P3], v: u32) -> Result<P3, MeshError> {
    points.get(v as usize).copied().ok_or(MeshError::Defect)
}

/// b − a, each component below 2^41.
pub(crate) fn difference(a: P3, b: P3) -> [i128; 3] {
    let along = |p: i64, q: i64| i128::from(q).saturating_sub(i128::from(p));
    [
        along(a.x.get(), b.x.get()),
        along(a.y.get(), b.y.get()),
        along(a.z.get(), b.z.get()),
    ]
}

/// u × v, each component below 2^83 for differences of grid points.
pub(crate) fn cross(u: [i128; 3], v: [i128; 3]) -> [i128; 3] {
    let [ux, uy, uz] = u;
    let [vx, vy, vz] = v;
    let minor = |p: i128, q: i128, r: i128, s: i128| {
        p.saturating_mul(q).saturating_sub(r.saturating_mul(s))
    };
    [
        minor(uy, vz, uz, vy),
        minor(uz, vx, ux, vz),
        minor(ux, vy, uy, vx),
    ]
}

/// u · v, below 2^84 for differences of grid points.
pub(crate) fn dot(u: [i128; 3], v: [i128; 3]) -> i128 {
    let [ux, uy, uz] = u;
    let [vx, vy, vz] = v;
    ux.saturating_mul(vx)
        .saturating_add(uy.saturating_mul(vy))
        .saturating_add(uz.saturating_mul(vz))
}

/// |b − a|², below 2^84.
pub(crate) fn squared(a: P3, b: P3) -> i128 {
    let d = difference(a, b);
    dot(d, d)
}

/// Whether a, b and c lie on one line, any two of them equal included.
pub(crate) fn collinear(a: P3, b: P3, c: P3) -> bool {
    cross(difference(a, b), difference(a, c)) == [0; 3]
}

/// Whether `p` lies on the segment from `a` to `b`, strictly between its ends.
pub(crate) fn inside_segment(a: P3, b: P3, p: P3) -> bool {
    let (along, to) = (difference(a, b), difference(a, p));
    cross(along, to) == [0; 3] && dot(along, to) > 0 && dot(along, to) < dot(along, along)
}

/// The least whole number of steps not below the square root of `n`: a distance whose square is
/// `n`, rounded up.
pub(crate) fn steps_up(n: i128) -> u64 {
    let n = n.unsigned_abs();
    let root = n.isqrt();
    let up = if root.saturating_mul(root) < n {
        root.saturating_add(1)
    } else {
        root
    };
    u64::try_from(up).unwrap_or(u64::MAX)
}
