//! Step 2 of healing (19.7): vertices joined within ε, and the faces that leaves made clean.
//!
//! What it does: [`weld()`] keys each distinct grid point by its Morton code and visits them in key
//! order. Each joins the lowest-keyed representative within ε, found among the candidates the cell
//! index gives and decided by exact squared distance; a point with none becomes a representative.
//! Then every face is rewritten onto the representatives and cleaned: a corner repeating the one
//! before it goes, a loop that passes a vertex twice is cut there into simple loops, and a loop
//! with no area is removed. A loop off one plane, or one that crosses itself, is split into a fan
//! from its lowest-keyed corner. A face repeated the same way round is kept once.
//!
//! Guarantees:
//!
//! - **Representatives never move, so clusters cannot chain.** A vertex moves at most ε, to a
//!   representative that was within ε of it; the log names every move, and
//!   [`WeldLog::movement`] is the largest, rounded up to whole steps.
//! - **Cells change nothing.** The cell index only finds candidates; the decision is the exact
//!   squared distance, so where the cells fall does not show.
//! - **Every face left is a simple, flat loop with area,** on distinct grid points, starting at its
//!   lowest-keyed corner, and no two are the same loop the same way round: what the mesh checker
//!   asks of a face (19.2).
//! - **Everything removed or split is listed,** with the input face it came from.
//! - **A weld distance of 0 joins only equal grid points,** which is what rounding imported
//!   polyhedra needs (24.8).
//!
//! Method: 19.7's step 2 (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`) on
//! `geom-bvh`'s Morton-sorted cell index (19.9); a loop is simple when no two of its sides meet
//! but at their shared corner, tested by exact orientation in the plane of its dominant axis.
//!
//! Cost: O(n log n) for n vertices, one cell search each; per face, linear in its corners for the
//! cleaning and flatness and quadratic for the crossing test of a face of more than three corners.

#![allow(
    clippy::many_single_char_names,
    reason = "points a, b, c and sides p–q, r–s, as in the orientation tests"
)]

use alloc::vec;
use alloc::vec::Vec;

use geom_bvh::{CellIndex, Point};
use geom_num::Sign;
use geom_num::grid::{P2, P3, Steps};
use geom_num::plane::Plane;
use geom_num::predicate::orient2d;
use geom_run::{Exec, WorkMeter};

use super::loops::{Loop, collinear, fan, lowest_first, point, split, squared, steps_up};
use super::{HealError, RawMesh};
use crate::exact::{MeshError, morton};

/// A vertex that moved: from its grid point to its representative's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Joined {
    /// Where it was.
    pub from: P3,
    /// Where it went.
    pub to: P3,
}

/// Why a face, or part of one, was removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    /// It had no area left: fewer than three corners, or all on one line.
    Degenerate,
    /// It repeated a face kept from this input face, the same way round.
    Repeated {
        /// The input face kept.
        of: u32,
    },
}

/// A face, or part of one, removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Removed {
    /// The input face.
    pub face: u32,
    /// Why.
    pub why: Removal,
}

/// A face split into a fan because it was no longer flat or no longer simple.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Split {
    /// The input face.
    pub face: u32,
    /// How many triangles it became.
    pub into: u32,
}

/// What the weld did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeldLog {
    /// Every vertex that moved, in key order.
    pub joined: Vec<Joined>,
    /// Every face or part of one removed, in input order.
    pub removed: Vec<Removed>,
    /// Every face split, in input order.
    pub split: Vec<Split>,
    /// The largest move, rounded up to whole steps: at most ε.
    pub movement: Steps,
}

/// A welded mesh, where each face came from, and what the weld did.
#[derive(Clone, Debug, PartialEq)]
pub struct Welded {
    /// The mesh: the representatives in key order, and the cleaned faces in input order.
    pub mesh: RawMesh,
    /// Each face's input face.
    pub source: Vec<u32>,
    /// Whether a corner of each face's input face moved.
    pub moved: Vec<bool>,
    /// What the weld did.
    pub log: WeldLog,
}

/// What a cleaned loop is.
enum Shape {
    /// No area: all its corners on one line.
    Degenerate,
    /// Flat and simple: a face as it is.
    Kept,
    /// Off one plane, or crossing itself: a fan.
    Split,
}

/// `raw` welded within `epsilon` steps and its faces cleaned (module documentation).
///
/// # Errors
///
/// [`HealError::Mesh`] for a stopped meter, a refused allocation, or more vertices than 32-bit
/// numbers name.
pub fn weld(
    raw: &RawMesh,
    epsilon: Steps,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Welded, HealError> {
    let (points, of_raw) = distinct(raw.vertices(), meter)?;
    let (rep, joined, worst) = representatives(&points, epsilon, exec, meter)?;
    let mut number = vec![0_u32; points.len()];
    let mut vertices = Vec::new();
    for ((i, &p), (&r, slot)) in (0_u32..).zip(&points).zip(rep.iter().zip(&mut number)) {
        if r == i {
            *slot = crate::exact::index(vertices.len())?;
            vertices.push(p);
        }
    }
    // Each raw vertex's welded vertex, and whether it moved to get there.
    let mut target = Vec::with_capacity(of_raw.len());
    for &q in &of_raw {
        let r = *rep.get(q as usize).ok_or(MeshError::Defect)?;
        let v = *number.get(r as usize).ok_or(MeshError::Defect)?;
        target.push((v, r != q));
    }
    let mut log = WeldLog {
        joined,
        movement: Steps(steps_up(worst)),
        ..WeldLog::default()
    };
    let mut kept: Vec<(Loop, u32, bool)> = Vec::new();
    for (face, pair) in (0_u32..).zip(raw.face_start().windows(2)) {
        let &[start, end] = pair else { continue };
        let corners = raw
            .corners()
            .get(start as usize..end as usize)
            .ok_or(MeshError::Defect)?;
        let mut looped = Vec::with_capacity(corners.len());
        let mut moved = false;
        for &c in corners {
            meter.charge(1)?;
            let &(v, m) = target.get(c as usize).ok_or(MeshError::Defect)?;
            looped.push(v);
            moved |= m;
        }
        for part in clean(&looped, face, &vertices, &mut log, meter)? {
            kept.push((part, face, moved));
        }
    }
    let kept = unrepeated(kept, &mut log.removed);
    let mut corners = Vec::new();
    let mut face_start = vec![0_u32];
    let (mut source, mut moved) = (Vec::new(), Vec::new());
    for (looped, face, m) in kept {
        corners.extend_from_slice(&looped);
        face_start.push(crate::exact::index(corners.len())?);
        source.push(face);
        moved.push(m);
    }
    log.removed.sort_by_key(|r| r.face);
    Ok(Welded {
        mesh: RawMesh::new(vertices, corners, face_start)?,
        source,
        moved,
        log,
    })
}

/// The distinct points of `vertices` in key order, and each vertex's place among them.
fn distinct(vertices: &[P3], meter: &mut WorkMeter<'_>) -> Result<(Vec<P3>, Vec<u32>), HealError> {
    let mut keyed: Vec<(u128, u32)> = Vec::new();
    keyed
        .try_reserve(vertices.len())
        .map_err(|_| MeshError::OutOfMemory)?;
    for (v, &p) in (0_u32..).zip(vertices) {
        keyed.push((morton(p), v));
    }
    keyed.sort_unstable();
    let mut points: Vec<P3> = Vec::new();
    let mut of_raw = vec![0_u32; vertices.len()];
    let mut last = None;
    for &(code, v) in &keyed {
        meter.charge(1)?;
        if last != Some(code) {
            points.push(point(vertices, v)?);
            last = Some(code);
        }
        let place = crate::exact::index(points.len().saturating_sub(1))?;
        *of_raw.get_mut(v as usize).ok_or(MeshError::Defect)? = place;
    }
    Ok((points, of_raw))
}

/// Each point's representative, the moves, and the largest move's square.
fn representatives(
    points: &[P3],
    epsilon: Steps,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<u32>, Vec<Joined>, i128), HealError> {
    let mut rep: Vec<u32> = (0..crate::exact::index(points.len())?).collect();
    let mut joined = Vec::new();
    let mut worst = 0_i128;
    if epsilon.get() == 0 {
        return Ok((rep, joined, worst));
    }
    let keyed: Vec<Point> = (0_u64..)
        .zip(points)
        .map(|(key, &at)| Point { key, at })
        .collect();
    let index = CellIndex::build(&keyed, exec, meter)?;
    let mut is_rep = vec![false; points.len()];
    for (i, &p) in (0_u32..).zip(points) {
        // Ascending keys, and only lower ones are representatives yet: the first is the lowest.
        let near = index.within(p, epsilon.get(), meter)?;
        let found = near.into_iter().find(|&k| {
            usize::try_from(k)
                .ok()
                .and_then(|k| is_rep.get(k))
                .is_some_and(|&r| r)
        });
        match found.and_then(|k| u32::try_from(k).ok()) {
            Some(k) => {
                let to = point(points, k)?;
                *rep.get_mut(i as usize).ok_or(MeshError::Defect)? = k;
                joined.push(Joined { from: p, to });
                worst = worst.max(squared(p, to));
            }
            None => *is_rep.get_mut(i as usize).ok_or(MeshError::Defect)? = true,
        }
    }
    Ok((rep, joined, worst))
}

/// The simple, flat loops with area that `looped`, a face of input face `face` on the welded
/// vertices, leaves: noted in `log` as removed or split.
fn clean(
    looped: &[u32],
    face: u32,
    points: &[P3],
    log: &mut WeldLog,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Loop>, HealError> {
    let mut out = Vec::new();
    for mut part in split(looped) {
        if part.len() < 3 {
            log.removed.push(Removed {
                face,
                why: Removal::Degenerate,
            });
            continue;
        }
        lowest_first(&mut part);
        match shape(&part, points, meter)? {
            Shape::Kept => out.push(part),
            Shape::Degenerate => log.removed.push(Removed {
                face,
                why: Removal::Degenerate,
            }),
            Shape::Split => {
                let triangles = fan(&part, points)?;
                log.split.push(Split {
                    face,
                    into: crate::exact::index(triangles.len())?,
                });
                out.extend(triangles);
            }
        }
    }
    Ok(out)
}

/// Whether `looped`, of distinct vertices and three or more, has area, and is flat and simple.
fn shape(looped: &[u32], points: &[P3], meter: &mut WorkMeter<'_>) -> Result<Shape, HealError> {
    let Some((&first, rest)) = looped.split_first() else {
        return Ok(Shape::Degenerate);
    };
    let a = point(points, first)?;
    let mut plane = None;
    for pair in rest.windows(2) {
        let &[b, c] = pair else { continue };
        let (b, c) = (point(points, b)?, point(points, c)?);
        if !collinear(a, b, c) {
            plane = Some(Plane::through(a, b, c)?);
            break;
        }
    }
    let Some(plane) = plane else {
        return Ok(Shape::Degenerate);
    };
    for &v in rest {
        meter.charge(1)?;
        if plane.side(point(points, v)?)? != Sign::Zero {
            return Ok(Shape::Split);
        }
    }
    if looped.len() == 3 || simple(looped, points, &plane, meter)? {
        Ok(Shape::Kept)
    } else {
        Ok(Shape::Split)
    }
}

/// Whether the flat loop `looped` on `plane` is simple: no two sides meet but two neighbours at
/// their shared corner, and no corner turns straight back. Tested in the plane of the normal's
/// largest component, onto which the loop's plane projects one to one.
fn simple(
    looped: &[u32],
    points: &[P3],
    plane: &Plane,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, HealError> {
    let [nx, ny, nz] = plane.n().map(i128::unsigned_abs);
    let axis = if nx >= ny && nx >= nz {
        0
    } else if ny >= nz {
        1
    } else {
        2
    };
    let mut flat = Vec::with_capacity(looped.len());
    for &v in looped {
        let p = point(points, v)?;
        let [x, y, z] = [p.x.get(), p.y.get(), p.z.get()];
        flat.push(match axis {
            0 => P2::new(y, z)?,
            1 => P2::new(z, x)?,
            _ => P2::new(x, y)?,
        });
    }
    let k = flat.len();
    let side = |i: usize| -> Option<(P2, P2)> {
        Some((*flat.get(i)?, *flat.get(i.checked_add(1)?.checked_rem(k)?)?))
    };
    for i in 0..k {
        let (p, q) = side(i).ok_or(MeshError::Defect)?;
        for j in i.saturating_add(1)..k {
            meter.charge(1)?;
            let (r, s) = side(j).ok_or(MeshError::Defect)?;
            let meets = if j == i.saturating_add(1) {
                folds(p, q, s)?
            } else if i == 0 && j == k.saturating_sub(1) {
                folds(q, p, r)?
            } else {
                meet(p, q, r, s)?
            };
            if meets {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// Whether the sides a → b and b → c, which share b, overlap: c lies on the ray from b back
/// through a.
fn folds(a: P2, b: P2, c: P2) -> Result<bool, HealError> {
    if orient2d(a, b, c)? != Sign::Zero {
        return Ok(false);
    }
    let along = |p: P2, q: P2| {
        let (x, y) = (
            i128::from(q.x.get()).saturating_sub(i128::from(p.x.get())),
            i128::from(q.y.get()).saturating_sub(i128::from(p.y.get())),
        );
        (x, y)
    };
    let ((ux, uy), (vx, vy)) = (along(b, a), along(b, c));
    Ok(ux.saturating_mul(vx).saturating_add(uy.saturating_mul(vy)) > 0)
}

/// Whether the closed segments p–q and r–s share a point.
fn meet(p: P2, q: P2, r: P2, s: P2) -> Result<bool, HealError> {
    let (d1, d2) = (orient2d(r, s, p)?, orient2d(r, s, q)?);
    let (d3, d4) = (orient2d(p, q, r)?, orient2d(p, q, s)?);
    let opposite = |a: Sign, b: Sign| {
        matches!(
            (a, b),
            (Sign::Positive, Sign::Negative) | (Sign::Negative, Sign::Positive)
        )
    };
    if opposite(d1, d2) && opposite(d3, d4) {
        return Ok(true);
    }
    Ok((d1 == Sign::Zero && between(r, s, p))
        || (d2 == Sign::Zero && between(r, s, q))
        || (d3 == Sign::Zero && between(p, q, r))
        || (d4 == Sign::Zero && between(p, q, s)))
}

/// Whether `p`, on the line through `a` and `b`, lies on the segment between them.
fn between(a: P2, b: P2, p: P2) -> bool {
    let within = |a: i64, b: i64, p: i64| a.min(b) <= p && p <= a.max(b);
    within(a.x.get(), b.x.get(), p.x.get()) && within(a.y.get(), b.y.get(), p.y.get())
}

/// `kept` without a loop repeated the same way round: the first is kept, each repeat listed.
fn unrepeated(kept: Vec<(Loop, u32, bool)>, removed: &mut Vec<Removed>) -> Vec<(Loop, u32, bool)> {
    let corners = |i: usize| kept.get(i).map(|k| &k.0);
    let mut order: Vec<usize> = (0..kept.len()).collect();
    order.sort_by(|&a, &b| corners(a).cmp(&corners(b)).then(a.cmp(&b)));
    let mut gone = vec![false; kept.len()];
    // Equal loops sort together, the earliest first: it is the one kept.
    for run in order.chunk_by(|&a, &b| corners(a) == corners(b)) {
        let Some((&first, rest)) = run.split_first() else {
            continue;
        };
        let of = kept.get(first).map_or(0, |k| k.1);
        for &i in rest {
            if let (Some(slot), Some(k)) = (gone.get_mut(i), kept.get(i)) {
                *slot = true;
                removed.push(Removed {
                    face: k.1,
                    why: Removal::Repeated { of },
                });
            }
        }
    }
    kept.into_iter()
        .zip(gone)
        .filter_map(|(k, g)| (!g).then_some(k))
        .collect()
}
