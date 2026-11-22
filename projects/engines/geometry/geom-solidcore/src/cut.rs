//! The engine's section: a flat-faced solid cut by a plane square to a grid axis, exactly, from the
//! kernel body's own rows.
//!
//! What it does: [`section`] answers `Engine::section` for [`crate::engine::SolidcoreEngine`].
//! The kernel's own parts find the curves: `geom_bool::operand::prepare` gives the solid's faces as
//! convex pieces on exact planes, each with its face's row; the cut plane becomes one convex piece,
//! a rectangle reaching a step past the solid's box; `geom_poly::cut::local_cuts` cuts each piece
//! by every piece that crosses it; and `geom_poly::merge::section` gives the exact curves where the
//! plane's piece meets the solid's, each edge with the pieces it lies on. This module keeps only
//! what those do not do: it decides first, from the solid's corners, whether the plane misses or
//! only touches the solid; it names each edge's face by its plane; it turns each edge so that the
//! material is on its left seen from where the plane's normal points; it joins the edges into loops,
//! one edge per face in a row; it checks every loop closed; and it puts the loops in their
//! canonical order.
//!
//! Guarantees:
//!
//! - **Exact.** Every point is the exact meeting of three planes the solid and the cut hold, given
//!   as a fraction of grid steps in lowest terms; nothing is rounded, and the cut carries the
//!   solid's exactness.
//! - **Touching or missing cuts nothing.** When no corner of the solid lies strictly on one side of
//!   the plane, the answer has no loops (chapter 18.4).
//! - **Every loop closes,** or the answer is a refusal; a point where more than two edges of the
//!   cut meet is unsupported, never guessed.
//! - **The same answer every time:** loops start at their least point, by x, then y, then z, and
//!   come in the order of those points.
//!
//! Not yet (each unsupported, as a value): a curved face; a plane not square to a grid axis; a
//! corner of the solid on the plane, where chapter 18.4's tie rules (a face in the plane is never a
//! cut, touching loops at a saddle) need the 2D work of the kernel's piece G389; a point whose
//! exact fraction does not fit 128 bits.
//!
//! Interim: this is geom-solidcore's own code over the kernel's parts until the kernel's own exact
//! sections of every body kind (piece G389, chapter 18.4) take the operation over; the interim
//! register (`tests/interim.json`) holds its row.
//!
//! Cost: `local_cuts` is O(n²) pairs of the solid's n convex pieces and the plane's one, and the
//! join O(e log e) for the e edges of the cut.
//!
//! SPDX-License-Identifier: Apache-2.0

use std::cmp::Ordering;
use std::collections::BTreeMap;

use geom_bool::operand::{BooleanInput, Operand, OperandKey, OperandKind, Selection, prepare};
use geom_num::plane::{Plane, Tpi};
use geom_num::{Exactness, Sign};
use geom_op::engine::{Cut, CutEdge, CutLoop, CutPoint, EngineError, GridPlane};
use geom_poly::convex::Piece;
use geom_poly::cut::local_cuts;
use geom_poly::merge::{Merge, section as meetings};
use geom_run::{Budget, WorkMeter};
use geom_topo::mass::Corner;
use geom_topo::query::corner;
use geom_topo::{Body, FaceIx, SurfaceKind, VertexIx};
use num_bigint::BigInt;

const OP: &str = "section";

fn refused(reason: impl std::fmt::Display) -> EngineError {
    EngineError::Refused {
        operation: OP,
        reason: reason.to_string(),
    }
}

fn unsupported(what: &str) -> EngineError {
    EngineError::Unsupported {
        operation: OP,
        what: what.to_owned(),
    }
}

/// The plane's axis and which way along it its normal points, when the normal lies along one.
fn axis_of(normal: [i128; 3]) -> Option<(usize, bool)> {
    match normal {
        [x, 0, 0] if x != 0 => Some((0, x > 0)),
        [0, y, 0] if y != 0 => Some((1, y > 0)),
        [0, 0, z] if z != 0 => Some((2, z > 0)),
        _ => None,
    }
}

/// The unit vector along `axis`, `sign` times.
fn unit(axis: usize, sign: i128) -> [i128; 3] {
    let mut v = [0; 3];
    v[axis] = sign;
    v
}

/// Two axes `(u, v)` in the plane square to `axis`, as signed unit vectors, with `u × v` pointing
/// the way the plane's normal does.
fn frame(axis: usize, up: bool) -> ([i128; 3], [i128; 3]) {
    // For +x, +y, +z: (y, z), (z, x), (x, y); for the opposite normal the pair is swapped.
    let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
    if up {
        (unit(a, 1), unit(b, 1))
    } else {
        (unit(b, 1), unit(a, 1))
    }
}

fn dot(a: [i128; 3], b: [i128; 3]) -> Option<i128> {
    a[0].checked_mul(b[0])?
        .checked_add(a[1].checked_mul(b[1])?)?
        .checked_add(a[2].checked_mul(b[2])?)
}

fn cross(a: [i128; 3], b: [i128; 3]) -> Option<[i128; 3]> {
    let m = |p: i128, q: i128, r: i128, s: i128| p.checked_mul(q)?.checked_sub(r.checked_mul(s)?);
    Some([
        m(a[1], b[2], a[2], b[1])?,
        m(a[2], b[0], a[0], b[2])?,
        m(a[0], b[1], a[1], b[0])?,
    ])
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// `plane` as a [`GridPlane`]: the points with `n · p = -d`, its coefficients with no common factor
/// and its normal the way it points.
fn carrier(plane: &Plane) -> Result<GridPlane, EngineError> {
    let n = plane.n();
    let offset = plane
        .d()
        .checked_neg()
        .ok_or_else(|| refused("a face's plane past 128 bits"))?;
    let g = [n[0], n[1], n[2], offset]
        .iter()
        .fold(0, |g, v| gcd(g, v.unsigned_abs()));
    let g = i128::try_from(g).map_err(|_| refused("a face's plane past 128 bits"))?;
    if g == 0 {
        return Err(refused("a face's plane with no normal"));
    }
    Ok(GridPlane {
        normal: n.map(|c| c / g),
        offset: offset / g,
    })
}

/// The exact point `t`, as a fraction of grid steps in lowest terms.
fn point_of(t: &Tpi) -> Result<CutPoint, EngineError> {
    if let Ok(g) = t.to_grid()
        && g.is_exact()
    {
        let p = g.value();
        return Ok(CutPoint {
            num: [p.x.get(), p.y.get(), p.z.get()].map(i128::from),
            den: 1,
        });
    }
    let too_wide = || unsupported("a point of the cut whose exact fraction does not fit 128 bits");
    let (x, w) = t.homogeneous().map_err(refused)?;
    let mut num = [0_i128; 3];
    for (n, c) in num.iter_mut().zip(&x) {
        *n = c.to_i128().ok_or_else(too_wide)?;
    }
    let mut den = w.to_i128().ok_or_else(too_wide)?;
    if den < 0 {
        den = den.checked_neg().ok_or_else(too_wide)?;
        for n in &mut num {
            *n = n.checked_neg().ok_or_else(too_wide)?;
        }
    }
    let g = num
        .iter()
        .fold(den.unsigned_abs(), |g, v| gcd(g, v.unsigned_abs()));
    let g = i128::try_from(g).map_err(|_| too_wide())?;
    if g == 0 {
        return Err(refused("a point with no weight"));
    }
    Ok(CutPoint {
        num: num.map(|n| n / g),
        den: den / g,
    })
}

/// `a` against `b` by x, then y, then z, exactly.
fn order(a: &CutPoint, b: &CutPoint) -> Ordering {
    for i in 0..3 {
        let l = BigInt::from(a.num[i]) * BigInt::from(b.den);
        let r = BigInt::from(b.num[i]) * BigInt::from(a.den);
        match l.cmp(&r) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

/// The sign of `(b - a) · u`, exactly.
fn along(a: &CutPoint, b: &CutPoint, u: [i128; 3]) -> Ordering {
    let mut sum = BigInt::from(0);
    for ((&bi, &ai), &ui) in b.num.iter().zip(&a.num).zip(&u) {
        let d = BigInt::from(bi) * BigInt::from(a.den) - BigInt::from(ai) * BigInt::from(b.den);
        sum += d * BigInt::from(ui);
    }
    sum.cmp(&BigInt::from(0))
}

/// Which side of `plane` each corner of `body` is on, and the box round them in grid steps.
fn corners(body: &Body, plane: &Plane) -> Result<(Vec<Sign>, [[i64; 3]; 2]), EngineError> {
    let a = body.arenas();
    let mut signs = Vec::new();
    let mut lo = [i64::MAX; 3];
    let mut hi = [i64::MIN; 3];
    for row in 0..a.vertices.len() {
        let v = VertexIx::new(row).ok_or_else(|| refused("a vertex row past the index"))?;
        let (sign, near) = match corner(body, v).map_err(refused)? {
            Corner::Grid(p) => (plane.side(p).map_err(refused)?, p),
            Corner::Tpi(t) => (
                plane.side_of_tpi(&t).map_err(refused)?,
                t.to_grid().map_err(refused)?.value(),
            ),
            Corner::Lpi(l) => (
                plane.side_of_lpi(&l).map_err(refused)?,
                l.to_grid().map_err(refused)?.value(),
            ),
        };
        signs.push(sign);
        for (i, c) in [near.x.get(), near.y.get(), near.z.get()]
            .into_iter()
            .enumerate()
        {
            lo[i] = lo[i].min(c);
            hi[i] = hi[i].max(c);
        }
    }
    Ok((signs, [lo, hi]))
}

/// The cut plane's piece: the rectangle of `plane` reaching a step past `bounds` on every side,
/// its sides in turn counter-clockwise seen from where the plane's normal points.
fn plane_piece(
    plane: &Plane,
    (u, v): ([i128; 3], [i128; 3]),
    [lo, hi]: [[i64; 3]; 2],
) -> Result<Piece, EngineError> {
    let wide = || refused("the solid's box past the grid");
    let span = |w: [i128; 3]| -> Option<(i128, i128)> {
        let (a, b) = (dot(w, lo.map(i128::from))?, dot(w, hi.map(i128::from))?);
        Some((a.min(b).checked_sub(1)?, a.max(b).checked_add(1)?))
    };
    let (u0, u1) = span(u).ok_or_else(wide)?;
    let (v0, v1) = span(v).ok_or_else(wide)?;
    let neg = |w: [i128; 3]| w.map(|c| -c);
    let side = |n: [i128; 3], d: i128| Plane::new(n, d).map_err(refused);
    Ok(Piece {
        face: *plane,
        bounds: vec![
            side(v, -v0)?,
            side(neg(u), u1)?,
            side(neg(v), v1)?,
            side(u, -u0)?,
        ],
    })
}

/// One edge of the cut, by the places of its ends among the meeting points.
#[derive(Clone, Copy, Debug)]
struct Run {
    start: usize,
    end: usize,
    face: GridPlane,
}

/// Where `plane` cuts the flat-faced solid `body` of `exactness`.
pub(crate) fn section(
    body: &Body,
    exactness: Exactness,
    plane: &GridPlane,
) -> Result<Cut, EngineError> {
    if plane.normal == [0; 3] {
        return Err(EngineError::Invalid {
            operation: OP,
            what: "a plane with a zero normal".to_owned(),
        });
    }
    let Some((axis, up)) = axis_of(plane.normal) else {
        return Err(unsupported("a plane not square to a grid axis"));
    };
    let a = body.arenas();
    for row in 0..a.faces.len() {
        let face = FaceIx::new(row).ok_or_else(|| refused("a face row past the index"))?;
        let f = a
            .faces
            .get(face)
            .ok_or_else(|| refused("a face out of its arena"))?;
        if !matches!(a.surfaces.get(f.surface), Some(SurfaceKind::Plane(_))) {
            return Err(unsupported("a solid with a curved face"));
        }
    }
    let offset = plane
        .offset
        .checked_neg()
        .ok_or_else(|| unsupported("a plane past the grid"))?;
    let cut = Plane::new(plane.normal, offset).map_err(|_| unsupported("a plane past the grid"))?;
    let (signs, bounds) = corners(body, &cut)?;
    let above = signs.contains(&Sign::Positive);
    let below = signs.contains(&Sign::Negative);
    if !(above && below) {
        return Ok(Cut {
            loops: Vec::new(),
            exactness,
        });
    }
    if signs.contains(&Sign::Zero) {
        return Err(unsupported(
            "a corner of the solid on the cut plane: chapter 18.4's tie rules wait for the kernel's piece G389",
        ));
    }
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let input = BooleanInput {
        operands: vec![Operand {
            key: OperandKey([1; 16]),
            body,
            kind: OperandKind::Solid,
            exactness,
        }],
        select: vec![Selection::Section],
        merge: Merge::RemoveRedundant,
        accept: Some(exactness),
    };
    let ready = prepare(&input, &mut meter).map_err(refused)?;
    let sign = if up { 1 } else { -1 };
    let mut polygons: Vec<Piece> = ready.polygons.iter().map(|p| p.piece.clone()).collect();
    polygons.push(plane_piece(&cut, frame(axis, up), bounds)?);
    let cuts = local_cuts(&polygons, &mut meter).map_err(|e| refused(format!("{e:?}")))?;
    // Each piece with its owner: 0 for the solid's, 1 for the plane's; and the face row of each
    // of the solid's.
    let plane_at = ready.polygons.len();
    let mut pieces: Vec<(Piece, usize)> = Vec::new();
    let mut rows: Vec<Option<u32>> = Vec::new();
    for (at, parts) in cuts.into_iter().enumerate() {
        let row = if at == plane_at {
            None
        } else {
            Some(
                *ready
                    .rows
                    .get(at)
                    .ok_or_else(|| refused("a polygon with no face"))?,
            )
        };
        for part in parts {
            pieces.push((part, usize::from(row.is_none())));
            rows.push(row);
        }
    }
    let curves = meetings(&pieces, &mut meter).map_err(refused)?;
    let points = curves
        .vertices
        .iter()
        .map(point_of)
        .collect::<Result<Vec<_>, _>>()?;
    let normal = unit(axis, sign);
    let mut runs: Vec<Run> = Vec::new();
    for (&(low, high), meets) in curves.edges.iter().zip(&curves.meets) {
        let mut on_plane = false;
        let mut faces: Vec<(u32, usize)> = Vec::new();
        for &m in meets {
            match rows.get(m) {
                Some(None) => on_plane = true,
                Some(Some(row)) => faces.push((*row, m)),
                None => return Err(refused("a meeting on no piece")),
            }
        }
        if !on_plane {
            continue;
        }
        faces.sort_unstable();
        faces.dedup_by_key(|(row, _)| *row);
        let [(_, piece)] = faces.as_slice() else {
            return Err(unsupported("an edge of the cut on more than one face"));
        };
        let face_plane = &pieces
            .get(*piece)
            .ok_or_else(|| refused("a meeting on no piece"))?
            .0
            .face;
        let face = carrier(face_plane)?;
        // Material lies against the face's normal: walking along n × n_f keeps it on the left.
        let u =
            cross(normal, face.normal).ok_or_else(|| refused("a face's plane past 128 bits"))?;
        let (p, q) = (
            points
                .get(low)
                .ok_or_else(|| refused("an edge past its points"))?,
            points
                .get(high)
                .ok_or_else(|| refused("an edge past its points"))?,
        );
        let (start, end) = match along(p, q, u) {
            Ordering::Greater => (low, high),
            Ordering::Less => (high, low),
            Ordering::Equal => return Err(refused("an edge of the cut across its face")),
        };
        runs.push(Run { start, end, face });
    }
    let loops = joined(&runs, &points)?;
    Ok(Cut { loops, exactness })
}

/// `runs` joined end to start into closed loops, an edge per face in a row, each loop from its
/// least point and the loops in the order of those points.
fn joined(runs: &[Run], points: &[CutPoint]) -> Result<Vec<CutLoop>, EngineError> {
    let mut from: BTreeMap<usize, usize> = BTreeMap::new();
    let mut into: BTreeMap<usize, usize> = BTreeMap::new();
    for (i, run) in runs.iter().enumerate() {
        if from.insert(run.start, i).is_some() || into.insert(run.end, i).is_some() {
            return Err(unsupported(
                "a point where more than two edges of the cut meet",
            ));
        }
    }
    let mut used = vec![false; runs.len()];
    let mut loops: Vec<Vec<Run>> = Vec::new();
    for first in 0..runs.len() {
        if used[first] {
            continue;
        }
        let mut ring = Vec::new();
        let mut at = first;
        loop {
            if used[at] {
                return Err(refused("a loop of the cut does not close"));
            }
            used[at] = true;
            let run = runs[at];
            ring.push(run);
            let Some(&next) = from.get(&run.end) else {
                return Err(refused("a loop of the cut does not close"));
            };
            if next == first {
                break;
            }
            at = next;
        }
        loops.push(ring);
    }
    let mut out = Vec::with_capacity(loops.len());
    for ring in loops {
        // One edge per face in a row: neighbours on one face lie on one line, and are one edge.
        let mut merged: Vec<Run> = Vec::with_capacity(ring.len());
        for run in ring {
            match merged.last_mut() {
                Some(last) if last.face == run.face => last.end = run.end,
                _ => merged.push(run),
            }
        }
        if merged.len() > 1
            && let (Some(first), Some(last)) = (merged.first().copied(), merged.last().copied())
            && first.face == last.face
        {
            merged.pop();
            if let Some(head) = merged.first_mut() {
                head.start = last.start;
            }
        }
        if merged.len() < 3 {
            return Err(refused("a loop of the cut with fewer than three edges"));
        }
        let least = (0..merged.len())
            .min_by(|&i, &j| order(&points[merged[i].start], &points[merged[j].start]))
            .unwrap_or(0);
        merged.rotate_left(least);
        let edges = merged
            .iter()
            .map(|r| CutEdge {
                start: points[r.start],
                end: points[r.end],
                face: r.face,
            })
            .collect();
        out.push(CutLoop { edges });
    }
    out.sort_by(|a, b| match (a.edges.first(), b.edges.first()) {
        (Some(x), Some(y)) => order(&x.start, &y.start),
        _ => Ordering::Equal,
    });
    Ok(out)
}
