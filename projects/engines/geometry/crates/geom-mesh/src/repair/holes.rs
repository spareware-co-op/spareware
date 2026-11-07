//! Step 3 of healing (19.7): cracks of no width stitched, and the holes left listed.
//!
//! What it does: a boundary edge is one an odd number of faces use, counted without direction,
//! since nothing is oriented yet. Every boundary vertex lying exactly inside a boundary edge is
//! put into each face along that edge, in order, until none is left: a T-junction, or the crack a
//! removed sliver leaves, closes without moving anything. The boundary edges left are walked into
//! loops, each from its lowest-keyed vertex, always on to the lowest-keyed neighbour, and cut where
//! they pass a vertex twice; each loop is returned with where it is and how wide.
//!
//! Why no loop is filled here: 19.7 closes a hole that fits a ball of diameter ε with a fan from
//! its lowest-keyed corner. After step 2 at the same ε no such hole is left. A representative is
//! never within ε of an earlier one, or it would have joined it, so any two vertices left are more
//! than ε apart, and a loop has at least three: it is wider than ε. The weld has already closed
//! every hole within ε, by joining its corners, at a move of at most ε, which its class counts.
//! What a hole within ε leaves after the weld is a crack of no width, and that is stitched.
//!
//! Guarantees:
//!
//! - **Nothing moves.** A stitch adds a vertex already there to an edge it already lies on.
//! - **Exact.** A vertex is inside an edge when the cross product is zero and the dot products put
//!   it strictly between the ends, on integers; widths are squared integer distances.
//! - **Every boundary left is listed,** with its lowest-keyed corner, its corner count and its
//!   width, the largest distance between two of its corners, rounded up to whole steps.
//! - **Logged.** Every stitch names the vertex and the edge it went into.
//!
//! Method: 19.7's step 3 (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`);
//! boundary edges as uses that do not cancel, after Manifold's `MergeMeshGLP` (consulted, see
//! [`crate::repair`]), counted modulo 2 because no facing is known yet. Every vertex has an even
//! number of boundary edges, since each face passes a vertex by two edges, so each walk closes.
//!
//! Cost: per round of stitching, a sort of the edge uses and a cell index of the boundary vertices
//! with one search per boundary edge (a long edge's search reaches far); each round stitches at
//! least one vertex or is the last. Then the square of each loop's corners for its width.

use alloc::vec::Vec;

use geom_bvh::{CellIndex, Point};
use geom_num::grid::{P3, Steps};
use geom_run::{Exec, WorkMeter};

use super::loops::{
    Face, Loop, by_edge, difference, dot, inside_segment, lowest_first, point, split, squared,
    steps_up, uses,
};
use super::{Boundary, HealError};
use crate::exact::{MeshError, index};

/// A vertex put into an edge it lay on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stitched {
    /// The vertex.
    pub vertex: P3,
    /// The edge's ends, the lower-keyed first.
    pub between: [P3; 2],
}

/// What step 3 did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HoleLog {
    /// Every stitch, in the order made.
    pub stitched: Vec<Stitched>,
}

/// Faces after step 3, the boundaries left open, and what was done.
pub(crate) struct Closed {
    pub faces: Vec<Face>,
    pub open: Vec<Boundary>,
    pub log: HoleLog,
}

/// `faces` on `points` with their cracks stitched, and the boundaries left.
pub(crate) fn close(
    points: &[P3],
    mut faces: Vec<Face>,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Closed, HealError> {
    let mut log = HoleLog::default();
    while stitch(points, &mut faces, &mut log, exec, meter)? {}
    let mut open = Vec::new();
    for mut looped in loops(&odd(&faces)?, meter)? {
        let wide = width(&looped, points, meter)?;
        lowest_first(&mut looped);
        open.push(Boundary {
            at: point(points, *looped.first().ok_or(MeshError::Defect)?)?,
            corners: index(looped.len())?,
            width: Steps(steps_up(wide)),
        });
    }
    Ok(Closed { faces, open, log })
}

/// The boundary edges of `faces`: those an odd number of faces use, as (lower, higher) ends.
fn odd(faces: &[Face]) -> Result<Vec<(u32, u32)>, HealError> {
    let all = uses(faces)?;
    Ok(by_edge(&all)
        .filter(|group| group.len() % 2 == 1)
        .filter_map(|group| group.first().map(|u| (u.low, u.high)))
        .collect())
}

/// Puts each boundary vertex lying inside a boundary edge into every face along that edge;
/// whether any was.
fn stitch(
    points: &[P3],
    faces: &mut [Face],
    log: &mut HoleLog,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, HealError> {
    let edges = odd(faces)?;
    let mut ends: Vec<u32> = edges.iter().flat_map(|&(a, b)| [a, b]).collect();
    ends.sort_unstable();
    ends.dedup();
    let mut keyed = Vec::new();
    for &v in &ends {
        keyed.push(Point {
            key: u64::from(v),
            at: point(points, v)?,
        });
    }
    let cells = CellIndex::build(&keyed, exec, meter)?;
    // Each edge with what lies inside it, in order from its lower end.
    let mut inside: Vec<((u32, u32), Vec<u32>)> = Vec::new();
    for &(low, high) in &edges {
        let (a, b) = (point(points, low)?, point(points, high)?);
        let along = difference(a, b);
        let mut on = Vec::new();
        for key in cells.within(middle(a, b)?, reach(a, b), meter)? {
            let w = u32::try_from(key).map_err(|_| MeshError::Defect)?;
            let p = point(points, w)?;
            if inside_segment(a, b, p) {
                on.push((dot(along, difference(a, p)), w, p));
            }
        }
        if on.is_empty() {
            continue;
        }
        on.sort_unstable_by_key(|&(d, w, _)| (d, w));
        for &(_, _, vertex) in &on {
            log.stitched.push(Stitched {
                vertex,
                between: [a, b],
            });
        }
        inside.push(((low, high), on.into_iter().map(|(_, w, _)| w).collect()));
    }
    if inside.is_empty() {
        return Ok(false);
    }
    for face in faces.iter_mut() {
        meter.charge(1)?;
        let looped = &face.corners;
        let mut out = Loop::with_capacity(looped.len());
        for (&u, &v) in looped.iter().zip(looped.iter().cycle().skip(1)) {
            out.push(u);
            let key = (u.min(v), u.max(v));
            if let Ok(at) = inside.binary_search_by_key(&key, |e| e.0)
                && let Some((_, on)) = inside.get(at)
            {
                if u < v {
                    out.extend(on.iter().copied());
                } else {
                    out.extend(on.iter().rev().copied());
                }
            }
        }
        face.corners = out;
    }
    Ok(true)
}

/// The grid point at or just below the middle of a and b, each coordinate rounded down.
fn middle(a: P3, b: P3) -> Result<P3, HealError> {
    let half = |p: i64, q: i64| {
        p.checked_add(q)
            .and_then(|s| s.checked_div_euclid(2))
            .ok_or(MeshError::Defect)
    };
    Ok(P3::new(
        half(a.x.get(), b.x.get())?,
        half(a.y.get(), b.y.get())?,
        half(a.z.get(), b.z.get())?,
    )?)
}

/// A radius about [`middle`] that holds the whole edge from a to b: half its length, rounded up,
/// and one step for the rounding of the middle (each coordinate at most half a step off, so less
/// than one step in all).
fn reach(a: P3, b: P3) -> u64 {
    steps_up(squared(a, b)).div_ceil(2).saturating_add(1)
}

/// `edges`, each used once, walked into loops: from the lowest edge's lower end, always on to the
/// lowest unused neighbour, until back at the start; each walk cut at a vertex it passes twice.
fn loops(edges: &[(u32, u32)], meter: &mut WorkMeter<'_>) -> Result<Vec<Loop>, HealError> {
    // Each edge from both ends: (from, to, edge), sorted.
    let mut ends: Vec<(u32, u32, u32)> = Vec::new();
    for (e, &(a, b)) in (0_u32..).zip(edges) {
        ends.push((a, b, e));
        ends.push((b, a, e));
    }
    ends.sort_unstable();
    let mut used = alloc::vec![false; edges.len()];
    let mut out = Vec::new();
    for (e, &(start, mut at)) in edges.iter().enumerate() {
        if used.get(e).copied().unwrap_or(true) {
            continue;
        }
        if let Some(slot) = used.get_mut(e) {
            *slot = true;
        }
        let mut walk = alloc::vec![start];
        while at != start {
            meter.charge(1)?;
            walk.push(at);
            let first = ends.partition_point(|&(from, _, _)| from < at);
            let next = ends
                .get(first..)
                .unwrap_or(&[])
                .iter()
                .take_while(|&&(from, _, _)| from == at)
                .find(|&&(_, _, edge)| used.get(edge as usize) == Some(&false));
            // Every vertex has an even number of boundary edges, so the walk can always go on.
            let Some(&(_, to, edge)) = next else {
                return Err(MeshError::Defect.into());
            };
            if let Some(slot) = used.get_mut(edge as usize) {
                *slot = true;
            }
            at = to;
        }
        out.extend(split(&walk).into_iter().filter(|l| l.len() >= 3));
    }
    Ok(out)
}

/// The square of the largest distance between two corners of `looped`.
fn width(looped: &[u32], points: &[P3], meter: &mut WorkMeter<'_>) -> Result<i128, HealError> {
    let mut corners = Vec::with_capacity(looped.len());
    for &v in looped {
        corners.push(point(points, v)?);
    }
    let mut widest = 0_i128;
    for (i, &p) in corners.iter().enumerate() {
        meter.charge(1)?;
        for &q in corners.get(i.saturating_add(1)..).unwrap_or(&[]) {
            widest = widest.max(squared(p, q));
        }
    }
    Ok(widest)
}
