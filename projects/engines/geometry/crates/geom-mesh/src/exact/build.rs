//! Makes an [`ExactMesh`] from polygons: the planes, the canonical order and the pairing of 19.2.
//!
//! What it does: [`build`] takes [`Polygons`], whose vertices are grid points or three planes and
//! whose faces are loops of vertices, and works out what the mesh stores. A face made from grid
//! points gets its plane from its area vector; each edge between grid points gets the plane through
//! it that holds the face's dominant axis. Faces and edges off the grid bring their planes with
//! them. The planes go into one sorted pool, the vertices and faces into canonical order, and each
//! half-edge finds its opposite, in exact radial order where more than two meet.
//!
//! Guarantees:
//!
//! - **The order of the input does not show.** The result is the same value whatever order the
//!   vertices and faces came in, and however each loop was turned.
//! - **The threads do not show.** Every parallel step goes through [`Exec`], whose results do not
//!   depend on its pool (23.4), and every sort is then made a total order.
//! - **One point, one form.** Three planes that meet on a grid point give that grid point, and
//!   repeated grid points become one vertex. Repeated three-plane points are the caller's: they
//!   become one vertex only with the same canonical planes, and otherwise fail the checker.
//! - **The result is checked.** It goes through [`ExactMesh::new`].
//!
//! Method: chapter 19.2's "Canonical order" and "Plane-based polygons"
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`). The half-edges are
//! grouped by counting sort on an edge's lower vertex, so pairing is linear in the half-edges;
//! radial order is 11.3's order of flaps about a line. A half-edge running against its line has
//! its solid on the counter-clockwise side, so it pairs with the next flap round, which is why fins
//! alternate cell sides (11.7).
//!
//! Cost: O(n log n) in the half-edges for the sorts of the planes and faces, with a few allocations
//! per array and none per face but for faces with holes. A face's plane takes up to three greatest
//! common divisors, an edge plane's one or two.

#![allow(
    clippy::many_single_char_names,
    reason = "a, b, c and d are points and n, u and v vectors, as in the formulas"
)]

use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_num::grid::{P3, Steps};
use geom_num::plane::{Plane, Tpi};
use geom_num::{Exactness, NumError, Sign};
use geom_op::NameId;
use geom_poly::order::{Flap, Line, about, along, normals_det};
use geom_poly::plane::canonical;
use geom_run::{ChunkPlan, Exec, Phase, RunError, WorkMeter};

use super::{
    ExactMesh, Facing, MeshError, MeshFault, MeshKind, NONE, PLAN, Parts, PropTable, Role, Run,
    Vertex, at, gcd, index, morton,
};

const FACE_PLANES: Phase = Phase::new("exact mesh: face planes");
const EDGE_PLANES: Phase = Phase::new("exact mesh: edge planes");
const PLANE_RANKS: Phase = Phase::new("exact mesh: rank planes");
const SORT_PLANES: Phase = Phase::new("exact mesh: sort planes");
const PLANE_RUNS: Phase = Phase::new("exact mesh: repeated planes");
const FACE_KEYS: Phase = Phase::new("exact mesh: face keys");
const SORT_FACES: Phase = Phase::new("exact mesh: sort faces");

/// How many items a sort's chunk holds.
const SORT_PLAN: ChunkPlan = ChunkPlan::fixed::<4096>();

/// A vertex as a builder gets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    /// A grid point.
    Grid(P3),
    /// The meeting of three planes.
    Planes([Plane; 3]),
}

/// One source of faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source {
    /// Its persistent name.
    pub name: NameId,
    /// What it is to the mesh.
    pub role: Role,
    /// The caller's flags.
    pub flags: u32,
}

/// Polygons as a builder gets them. Only `vertices`, `corners`, `loop_start`, `face_loops`,
/// `face_run` and `runs` are needed; an empty optional array means none is given.
#[derive(Clone, Debug, PartialEq)]
pub struct Polygons {
    /// The vertices, in any order.
    pub vertices: Vec<Corner>,
    /// Each corner's vertex, loop after loop.
    pub corners: Vec<u32>,
    /// The corners of loop `l` are `loop_start[l]..loop_start[l + 1]`.
    pub loop_start: Vec<u32>,
    /// The loops of face `f` are `face_loops[f]..face_loops[f + 1]`; the first is the outer loop.
    pub face_loops: Vec<u32>,
    /// Each face's source, in `runs`.
    pub face_run: Vec<u32>,
    /// The sources, in any order.
    pub runs: Vec<Source>,
    /// Each face's plane where it has one to give, facing outward; a face with a corner off the
    /// grid must. Empty, or one per face.
    pub face_plane: Vec<Option<Plane>>,
    /// The plane of the edge each corner starts, facing into the face, where it has one to give;
    /// an edge with an end off the grid must. Empty, or one per corner.
    pub edge_plane: Vec<Option<Plane>>,
    /// Each corner's row of `props`, or [`NONE`]. Empty, or one per corner.
    pub corner_prop: Vec<u32>,
    /// Display data.
    pub props: PropTable,
    /// Each face's proven distance from its source. Empty, or one per face.
    pub face_dev: Vec<Steps>,
    /// How far the mesh is from what it stands for.
    pub class: Exactness,
}

/// A plane as it stands before the pool is sorted: reduced, and whether that turned it.
#[derive(Clone, Copy)]
struct Turned {
    n: [i128; 3],
    d: i128,
    flipped: bool,
}

impl Turned {
    /// The canonical form of a plane given.
    fn given(plane: &Plane) -> Result<Self, MeshError> {
        let c = canonical(plane)?;
        Ok(Self {
            n: c.plane.n(),
            d: c.plane.d(),
            flipped: c.flipped,
        })
    }

    /// The plane with normal `n` through `on`, reduced so that its normal's first non-zero
    /// component is positive. `n` must be a multiple of a primitive normal: dividing by its own
    /// divisor leaves that, and a plane through a grid point with a primitive normal has no
    /// common factor left. [`pool`] checks each plane's budget as it joins.
    fn reduced(n: [i128; 3], on: P3) -> Result<Self, MeshError> {
        let [x, y, z] = n;
        let first = gcd(x.unsigned_abs(), y.unsigned_abs());
        let divisor = if first == 1 {
            1
        } else {
            gcd(first, z.unsigned_abs())
        };
        let n = if divisor == 1 {
            n
        } else {
            let by = i128::try_from(divisor).map_err(|_| NumError::BudgetExceeded)?;
            n.map(|v| part(v, by))
        };
        let flipped = n
            .iter()
            .find(|v| **v != 0)
            .is_some_and(|first| first.is_negative());
        let n = if flipped {
            n.map(i128::saturating_neg)
        } else {
            n
        };
        let [a, b, c] = n;
        // Below 2^83 · 2^40 each, so the sum is far inside `i128`.
        let d = a
            .wrapping_mul(p_x(on))
            .wrapping_add(b.wrapping_mul(p_y(on)))
            .wrapping_add(c.wrapping_mul(p_z(on)))
            .wrapping_neg();
        Ok(Self { n, d, flipped })
    }
}

/// `v / by`, which is whole, in 64 bits where both fit: a 128-bit division is a hundred times the
/// cost, and most coefficients are small.
fn part(v: i128, by: i128) -> i128 {
    match (i64::try_from(v), i64::try_from(by)) {
        (Ok(a), Ok(b)) => a.checked_div(b).map_or(0, i128::from),
        _ => v.checked_div(by).unwrap_or(0),
    }
}

fn p_x(p: P3) -> i128 {
    i128::from(p.x.get())
}

fn p_y(p: P3) -> i128 {
    i128::from(p.y.get())
}

fn p_z(p: P3) -> i128 {
    i128::from(p.z.get())
}

/// `b − a`.
fn minus(b: P3, a: P3) -> [i128; 3] {
    [
        p_x(b).wrapping_sub(p_x(a)),
        p_y(b).wrapping_sub(p_y(a)),
        p_z(b).wrapping_sub(p_z(a)),
    ]
}

/// u × v, for differences of grid points: each is below 2^41 and each product below 2^82, so
/// wrapping steps are exact.
fn cross(u: [i128; 3], v: [i128; 3]) -> [i128; 3] {
    let [a, b, c] = u;
    let [x, y, z] = v;
    let term =
        |p: i128, q: i128, r: i128, s: i128| p.wrapping_mul(q).wrapping_sub(r.wrapping_mul(s));
    [term(b, z, c, y), term(c, x, a, z), term(a, y, b, x)]
}

/// Which axis of `n` is largest, the lowest of equals.
fn dominant(n: [i128; 3]) -> usize {
    let [x, y, z] = n.map(i128::unsigned_abs);
    if x >= y && x >= z {
        0
    } else if y >= z {
        1
    } else {
        2
    }
}

/// The plane through `a` and `b` that holds axis `k`, facing to the left of a to b seen from `n`'s
/// side, which a face with normal `n` lies on. For the unit vector e along `k`, `u` from `a` to
/// `b`, and m = e × u, m · (n × u) = |u|² `n_k`, so m looks to the left where `n_k` is positive.
fn edge_plane(a: P3, b: P3, n: [i128; 3], k: usize) -> Result<Turned, MeshError> {
    let [x, y, z] = minus(b, a);
    let m = match k {
        0 => [0, z.wrapping_neg(), y],
        1 => [z, 0, x.wrapping_neg()],
        _ => [y.wrapping_neg(), x, 0],
    };
    let m = if n.get(k).copied().unwrap_or(0).is_negative() {
        m.map(i128::wrapping_neg)
    } else {
        m
    };
    Turned::reduced(m, a)
}

/// The vertices once the grid points are sorted and merged, and what each input vertex became.
struct Vertices {
    /// The distinct grid points, by Morton code.
    grid: Vec<P3>,
    /// The input's three-plane points, each as canonical planes with their rows still to come.
    implicit: Vec<[Plane; 3]>,
    /// Each input vertex: a row of `grid`, or `grid.len()` plus a row of `implicit`.
    of_input: Vec<u32>,
    /// Each input vertex's grid point, where it has one.
    point: Vec<Option<P3>>,
}

/// Sorts the grid points and turns any three-plane point on the grid into one.
fn vertices(input: &Polygons, meter: &mut WorkMeter<'_>) -> Result<Vertices, MeshError> {
    let mut keyed = Vec::with_capacity(input.vertices.len());
    let mut implicit = Vec::new();
    let mut point = Vec::with_capacity(input.vertices.len());
    let mut sides: Vec<Option<u32>> = Vec::with_capacity(input.vertices.len());
    for (v, corner) in (0_u32..).zip(&input.vertices) {
        meter.charge(1)?;
        let on_grid = match corner {
            Corner::Grid(p) => Some(*p),
            Corner::Planes([p, q, r]) => {
                let rounded = Tpi::new(*p, *q, *r)?.to_grid()?;
                rounded.is_exact().then(|| rounded.value())
            }
        };
        point.push(on_grid);
        match (on_grid, corner) {
            (Some(p), _) => {
                keyed.push((morton(p), v));
                sides.push(None);
            }
            (None, Corner::Planes([p, q, r])) => {
                sides.push(Some(index(implicit.len())?));
                implicit.push([
                    canonical(p)?.plane,
                    canonical(q)?.plane,
                    canonical(r)?.plane,
                ]);
            }
            (None, Corner::Grid(_)) => return Err(MeshFault::Shape.into()),
        }
    }
    keyed.sort_unstable();
    let mut grid = Vec::with_capacity(keyed.len());
    let mut of_input = alloc::vec![0_u32; input.vertices.len()];
    let mut last = None;
    for (key, v) in keyed {
        if last != Some(key) {
            grid.push((*at(&point, v)?).ok_or(MeshFault::Shape)?);
            last = Some(key);
        }
        *of_input.get_mut(v as usize).ok_or(MeshFault::Shape)? =
            index(grid.len())?.saturating_sub(1);
    }
    let base = index(grid.len())?;
    for (slot, side) in of_input.iter_mut().zip(sides) {
        if let Some(i) = side {
            *slot = base.saturating_add(i);
        }
    }
    Ok(Vertices {
        grid,
        implicit,
        of_input,
        point,
    })
}

/// The offsets `array[i]..array[i + 1]` of item `i`.
pub(super) fn span(array: &[u32], i: u32) -> Result<(u32, u32), MeshFault> {
    let start = *at(array, i)?;
    let end = *at(array, i.checked_add(1).ok_or(MeshFault::Shape)?)?;
    Ok((start, end))
}

/// A slice by two offsets.
pub(super) fn cut(items: &[u32], (start, end): (u32, u32)) -> Result<&[u32], MeshFault> {
    items
        .get(start as usize..end as usize)
        .ok_or(MeshFault::Shape)
}

/// Checks that the input's arrays fit together; empty optional arrays stand for none given.
fn fits(input: &Polygons) -> Result<(u32, u32), MeshError> {
    let faces = index(input.face_loops.len().saturating_sub(1))?;
    let loops = index(input.loop_start.len().saturating_sub(1))?;
    let corners = input.corners.len();
    let one_per = |len: usize, wanted: usize| len == 0 || len == wanted;
    let rising = |array: &[u32]| array.windows(2).all(|p| p.first() <= p.last());
    let shaped = input.face_loops.last() == Some(&loops)
        && input.loop_start.last().map(|&l| l as usize) == Some(corners)
        && input.face_loops.first() == Some(&0)
        && input.loop_start.first() == Some(&0)
        && rising(&input.face_loops)
        && rising(&input.loop_start)
        && input.face_run.len() == faces as usize
        && one_per(input.face_plane.len(), faces as usize)
        && one_per(input.face_dev.len(), faces as usize)
        && one_per(input.edge_plane.len(), corners)
        && one_per(input.corner_prop.len(), corners);
    if !shaped {
        return Err(MeshFault::Shape.into());
    }
    for looped in 0..loops {
        let (start, end) = span(&input.loop_start, looped)?;
        if end.saturating_sub(start) < 3 {
            return Err(MeshFault::ShortLoop { looped }.into());
        }
    }
    Ok((faces, loops))
}

/// Where each corner goes round its loop: the corner after it.
fn next_corner(corner: u32, (start, end): (u32, u32)) -> u32 {
    let next = corner.saturating_add(1);
    if next == end { start } else { next }
}

/// Turns the phases' errors into the mesh's.
fn phase<T>(result: Result<T, RunError<MeshError>>) -> Result<T, MeshError> {
    result.map_err(MeshError::from)
}

/// One face's plane: the one given, or from its outer loop's area vector.
fn face_plane(input: &Polygons, verts: &Vertices, face: u32) -> Result<Turned, MeshError> {
    if let Some(plane) = input.face_plane.get(face as usize).copied().flatten() {
        return Turned::given(&plane);
    }
    let (first_loop, _) = span(&input.face_loops, face)?;
    let (start, end) = span(&input.loop_start, first_loop)?;
    let grid = |corner: u32| -> Result<P3, MeshError> {
        let v = *at(&input.corners, corner)?;
        (*at(&verts.point, v)?).ok_or(MeshError::FacePlaneNeeded { face })
    };
    let base = grid(start)?;
    let mut sum = [0_i128; 3];
    let mut last = minus(grid(start.saturating_add(1))?, base);
    for corner in start.saturating_add(2)..end {
        let u = minus(grid(corner)?, base);
        let [x, y, z] = cross(last, u);
        let [a, b, c] = sum;
        // 2^32 corners of at most 2^83 stay below 2^115.
        sum = [a.wrapping_add(x), b.wrapping_add(y), c.wrapping_add(z)];
        last = u;
    }
    if sum == [0; 3] {
        return Err(MeshFault::DegenerateFace { face }.into());
    }
    Turned::reduced(sum, base)
}

/// Each corner's face, and the corner after it.
fn corner_links(input: &Polygons, faces: u32) -> Result<(Vec<u32>, Vec<u32>), MeshFault> {
    let mut face_of = alloc::vec![0_u32; input.corners.len()];
    let mut after = alloc::vec![0_u32; input.corners.len()];
    for face in 0..faces {
        let (first, last) = span(&input.face_loops, face)?;
        for looped in first..last {
            let corners = span(&input.loop_start, looped)?;
            for corner in corners.0..corners.1 {
                *face_of.get_mut(corner as usize).ok_or(MeshFault::Shape)? = face;
                *after.get_mut(corner as usize).ok_or(MeshFault::Shape)? =
                    next_corner(corner, corners);
            }
        }
    }
    Ok((face_of, after))
}

/// One corner's edge plane: the one given, or the plane through its edge along the face's
/// dominant axis.
fn corner_plane(
    input: &Polygons,
    verts: &Vertices,
    faces: &[Turned],
    links: (&[u32], &[u32]),
    corner: u32,
) -> Result<Turned, MeshError> {
    if let Some(plane) = input.edge_plane.get(corner as usize).copied().flatten() {
        return Turned::given(&plane);
    }
    let grid = |c: u32| -> Result<Option<P3>, MeshFault> {
        at(&verts.point, *at(&input.corners, c)?).copied()
    };
    let (Some(a), Some(b)) = (grid(corner)?, grid(*at(links.1, corner)?)?) else {
        return Err(MeshError::EdgePlaneNeeded { corner });
    };
    if a == b {
        return Err(MeshFault::ZeroEdge { half_edge: corner }.into());
    }
    let turned = at(faces, *at(links.0, corner)?)?;
    let n = if turned.flipped {
        turned.n.map(i128::wrapping_neg)
    } else {
        turned.n
    };
    edge_plane(a, b, n, dominant(n))
}

/// A value's place in 64 bits: never out of order, and exact below 2^56. The sign, then the
/// magnitude's width, then its top 56 bits, so that offsets of any size stay apart.
fn rank(v: i128) -> u64 {
    let magnitude = v.unsigned_abs();
    let width = u128::BITS.saturating_sub(magnitude.leading_zeros());
    let top = u64::try_from(magnitude >> width.saturating_sub(56)).unwrap_or(0);
    let key = (u64::from(width) << 56) | top;
    if v.is_negative() {
        i64::MAX.cast_unsigned().saturating_sub(key)
    } else {
        (1 << 63) | key
    }
}

/// The pool: every plane in order, and the row of each plane given, in the order given.
fn pool(
    given: &[&[Turned]],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<Plane>, Vec<u32>), MeshError> {
    let plain: Vec<&Turned> = given.iter().copied().flatten().collect();
    let keyed: Result<Vec<[u64; 4]>, RunError<MeshError>> =
        exec.try_map(meter, PLANE_RANKS, PLAN, &plain, |_, t, _| {
            let [x, y, z] = t.n;
            Ok([rank(x), rank(y), rank(z), rank(t.d)])
        });
    let ranks = phase(keyed)?;
    let order: Result<Vec<usize>, RunError<MeshError>> =
        exec.sort_stable(meter, SORT_PLANES, SORT_PLAN, &ranks, |r| *r);
    let mut order = phase(order)?;
    let whole = |i: usize| plain.get(i).map(|t| (t.n, t.d));
    // Equal ranks are rare but for repeated planes, so most runs are one plane.
    let tied: Result<Vec<bool>, RunError<MeshError>> =
        exec.try_map(meter, PLANE_RUNS, PLAN, &order, |j, &i, _| {
            let before = j.checked_sub(1).and_then(|b| order.get(b)).copied();
            Ok(before.is_some_and(|b| ranks.get(b) == ranks.get(i)))
        });
    let tied = phase(tied)?;
    let mut start = 0;
    for end in 1..=order.len() {
        if tied.get(end) != Some(&true) {
            let group = order.get_mut(start..end).ok_or(MeshFault::Shape)?;
            group.sort_unstable_by_key(|&i| whole(i));
            start = end;
        }
    }

    let fresh: Result<Vec<bool>, RunError<MeshError>> =
        exec.try_map(meter, PLANE_RUNS, PLAN, &order, |j, &i, _| {
            let before = j.checked_sub(1).and_then(|b| order.get(b)).copied();
            Ok(before.is_none_or(|b| whole(b) != whole(i)))
        });
    let fresh = phase(fresh)?;
    let mut rows = alloc::vec![0_u32; plain.len()];
    let mut out: Vec<Plane> = Vec::new();
    for (&i, &new) in order.iter().zip(&fresh) {
        if new {
            let (n, d) = whole(i).ok_or(MeshFault::Shape)?;
            out.push(Plane::new(n, d)?);
        }
        *rows.get_mut(i).ok_or(MeshFault::Shape)? = index(out.len())?.saturating_sub(1);
    }
    Ok((out, rows))
}

/// The lowest three rows of `rows`, in order of triples, whose planes meet in a point.
fn lowest_triple(rows: &[u32], pool: &[Plane]) -> Result<Option<[u32; 3]>, MeshError> {
    for (i, &a) in rows.iter().enumerate() {
        for (j, &b) in rows.iter().enumerate().skip(i.saturating_add(1)) {
            for &c in rows.iter().skip(j.saturating_add(1)) {
                let (p, q, r) = (at(pool, a)?, at(pool, b)?, at(pool, c)?);
                if normals_det(p, q, r)? != Sign::Zero {
                    return Ok(Some([a, b, c]));
                }
            }
        }
    }
    Ok(None)
}

/// Each three-plane point's rows, sorted, and where each went: its key is the lowest triple of
/// independent planes through it among those of its faces and edges (19.2), so one point has one
/// key however it was made.
fn implicit_keys(
    verts: &Vertices,
    corner_vertex: &[u32],
    corner_rows: &[(u32, u32)],
    own_rows: &[u32],
    pool: &[Plane],
) -> Result<(Vec<[u32; 3]>, Vec<u32>), MeshError> {
    let base = index(verts.grid.len())?;
    let count = verts.implicit.len();
    let mut around: Vec<BTreeSet<u32>> = alloc::vec![BTreeSet::new(); count];
    for (i, set) in around.iter_mut().enumerate() {
        set.extend(own_rows.iter().skip(i.saturating_mul(3)).take(3));
    }
    for (&v, &(face_row, edge_row)) in corner_vertex.iter().zip(corner_rows) {
        if let Some(i) = v.checked_sub(base) {
            let set = around.get_mut(i as usize).ok_or(MeshFault::Shape)?;
            set.insert(face_row);
            set.insert(edge_row);
        }
    }
    let mut keys = Vec::with_capacity(count);
    for (set, i) in around.iter().zip(0_u32..) {
        let rows: Vec<u32> = set.iter().copied().collect();
        let triple = lowest_triple(&rows, pool)?.ok_or(MeshFault::PlanesDoNotMeet {
            vertex: base.saturating_add(i),
        })?;
        keys.push((triple, i));
    }
    keys.sort_unstable();
    let mut triples: Vec<[u32; 3]> = Vec::with_capacity(count);
    let mut place = alloc::vec![0_u32; count];
    for (triple, i) in keys {
        if triples.last() != Some(&triple) {
            triples.push(triple);
        }
        *place.get_mut(i as usize).ok_or(MeshFault::Shape)? =
            index(triples.len())?.saturating_sub(1);
    }
    Ok((triples, place))
}

/// The loop's lowest vertex, and the first corner it is at.
fn lowest_corner(vertex: &[u32], (start, end): (u32, u32)) -> Result<(u32, u32), MeshFault> {
    let run = cut(vertex, (start, end))?;
    let mut best: Option<(u32, u32)> = None;
    for (&v, corner) in run.iter().zip(start..) {
        if best.is_none_or(|(low, _)| v < low) {
            best = Some((v, corner));
        }
    }
    best.ok_or(MeshFault::Shape)
}

/// One face on its way to canonical order: what its order depends on first.
#[derive(Clone, Copy, PartialEq, Eq)]
struct FaceKey {
    /// The rank of its run.
    run: u32,
    /// Its lowest vertex over all its loops.
    lowest: u32,
    /// The two vertices after its outer loop's lowest.
    next: [u32; 2],
    /// Its outer loop's lowest corner.
    start: u32,
    /// The face, as it was given.
    face: u32,
}

/// One face's key.
fn face_key(
    input: &Polygons,
    vertex: &[u32],
    face: u32,
    run_rank: &[u32],
) -> Result<FaceKey, MeshFault> {
    let loops = span(&input.face_loops, face)?;
    let outer = span(&input.loop_start, loops.0)?;
    let (low, start) = lowest_corner(vertex, outer)?;
    let mut lowest = low;
    for looped in loops.0.saturating_add(1)..loops.1 {
        lowest = lowest.min(lowest_corner(vertex, span(&input.loop_start, looped)?)?.0);
    }
    let second = next_corner(start, outer);
    let third = next_corner(second, outer);
    Ok(FaceKey {
        run: *at(run_rank, *at(&input.face_run, face)?)?,
        lowest,
        next: [*at(vertex, second)?, *at(vertex, third)?],
        start,
        face,
    })
}

/// A face's loops in canonical order, each as `(loop, lowest corner)`: the outer loop, then the
/// holes by lowest vertex.
fn ordered_loops(
    input: &Polygons,
    vertex: &[u32],
    key: &FaceKey,
) -> Result<Vec<(u32, u32)>, MeshFault> {
    let loops = span(&input.face_loops, key.face)?;
    let mut holes = Vec::with_capacity(loops.1.saturating_sub(loops.0) as usize);
    for looped in loops.0.saturating_add(1)..loops.1 {
        let (low, corner) = lowest_corner(vertex, span(&input.loop_start, looped)?)?;
        holes.push((low, looped, corner));
    }
    holes.sort_unstable();
    let mut ordered = Vec::with_capacity(holes.len().saturating_add(1));
    ordered.push((loops.0, key.start));
    ordered.extend(
        holes
            .into_iter()
            .map(|(_, looped, corner)| (looped, corner)),
    );
    Ok(ordered)
}

/// One half-edge's use of its edge.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Use {
    /// The edge's higher vertex.
    pub(super) high: u32,
    /// The half-edge.
    pub(super) half_edge: u32,
    /// Whether it runs from the lower vertex to the higher.
    pub(super) forward: bool,
}

/// Each half-edge's end vertex.
pub(super) fn ends(parts: &Parts) -> Result<Vec<u32>, MeshFault> {
    let mut out = Vec::with_capacity(parts.he_vert.len());
    for pair in parts.loop_start.windows(2) {
        let &[start, end] = pair else {
            return Err(MeshFault::Shape);
        };
        let run = cut(&parts.he_vert, (start, end))?;
        for &v in run.iter().skip(1) {
            out.push(v);
        }
        if let Some(&v) = run.first() {
            out.push(v);
        }
    }
    Ok(out)
}

/// The half-edges around each edge: `first[v]..first[v + 1]` are the uses whose lower vertex is
/// `v`, each bucket sorted by higher vertex. A counting sort, linear in the half-edges.
pub(super) fn grouped(
    starts: &[u32],
    ends: &[u32],
    vertices: usize,
) -> Result<(Vec<u32>, Vec<Use>), MeshFault> {
    let mut first = alloc::vec![0_u32; vertices.saturating_add(1)];
    for (&a, &b) in starts.iter().zip(ends) {
        let slot = first
            .get_mut((a.min(b) as usize).saturating_add(1))
            .ok_or(MeshFault::Shape)?;
        *slot = slot.saturating_add(1);
    }
    let mut sum = 0_u32;
    for slot in &mut first {
        sum = sum.saturating_add(*slot);
        *slot = sum;
    }
    let mut next = first.clone();
    let blank = Use {
        high: 0,
        half_edge: 0,
        forward: false,
    };
    let mut uses = alloc::vec![blank; starts.len()];
    for ((&a, &b), half_edge) in starts.iter().zip(ends).zip(0_u32..) {
        let slot = next.get_mut(a.min(b) as usize).ok_or(MeshFault::Shape)?;
        *uses.get_mut(*slot as usize).ok_or(MeshFault::Shape)? = Use {
            high: a.max(b),
            half_edge,
            forward: a < b,
        };
        *slot = slot.saturating_add(1);
    }
    for pair in first.windows(2) {
        let &[start, end] = pair else {
            return Err(MeshFault::Shape);
        };
        uses.get_mut(start as usize..end as usize)
            .ok_or(MeshFault::Shape)?
            .sort_unstable();
    }
    Ok((first, uses))
}

/// A plane through `v` that crosses `line`.
pub(super) fn crossing(parts: &Parts, line: &Line, v: u32) -> Result<Plane, MeshError> {
    let [p, q] = line.planes();
    let candidates: [Option<Plane>; 3] = match *at(&parts.verts, v)? {
        Vertex::Grid(c) => [
            Plane::new([1, 0, 0], p_x(c).wrapping_neg()).ok(),
            Plane::new([0, 1, 0], p_y(c).wrapping_neg()).ok(),
            Plane::new([0, 0, 1], p_z(c).wrapping_neg()).ok(),
        ],
        Vertex::Planes(rows) => rows.map(|row| parts.planes.get(row as usize).copied()),
    };
    for plane in candidates.iter().flatten() {
        if normals_det(&p, &q, plane)? != Sign::Zero {
            return Ok(*plane);
        }
    }
    Err(MeshFault::Shape.into())
}

/// The half-edges of one edge from `low` to `high`, in radial order about its line: each with
/// whether it runs against the line.
pub(super) fn radial(
    parts: &Parts,
    faces_of: &[u32],
    uses: &[Use],
    low: u32,
    high: u32,
) -> Result<Vec<(u32, bool)>, MeshError> {
    let first = *uses.first().ok_or(MeshFault::Shape)?;
    let facing_of = |half_edge: u32| -> Result<Facing, MeshFault> {
        at(&parts.face_plane, *at(faces_of, half_edge)?).copied()
    };
    let plane = |row: u32| at(&parts.planes, row).copied();
    let line = Line::new(
        plane(facing_of(first.half_edge)?.row)?,
        plane(at(&parts.he_plane, first.half_edge)?.row)?,
    )?;
    let low_first = along(
        &line,
        &crossing(parts, &line, low)?,
        &crossing(parts, &line, high)?,
    )? == Ordering::Less;
    let mut flaps = Vec::with_capacity(uses.len());
    for &u in uses {
        let facing = facing_of(u.half_edge)?;
        let against = u.forward != low_first;
        flaps.push((
            Flap {
                plane: plane(facing.row)?,
                reversed: facing.flipped != against,
            },
            u.half_edge,
            against,
        ));
    }
    let (reference, _, _) = *flaps.first().ok_or(MeshFault::Shape)?;
    let mut failed = None;
    flaps.sort_by(|a, b| {
        about(&line, &reference, &a.0, &b.0)
            .unwrap_or_else(|e| {
                failed = Some(e);
                Ordering::Equal
            })
            .then(a.1.cmp(&b.1))
    });
    if let Some(e) = failed {
        return Err(e.into());
    }
    Ok(flaps.iter().map(|&(_, h, against)| (h, against)).collect())
}

/// The face of each half-edge.
pub(super) fn faces_of(parts: &Parts) -> Result<Vec<u32>, MeshFault> {
    let mut out = alloc::vec![0_u32; parts.he_vert.len()];
    let faces = u32::try_from(parts.face_plane.len()).map_err(|_| MeshFault::Shape)?;
    for face in 0..faces {
        let loops = span(&parts.face_loops, face)?;
        let start = *at(&parts.loop_start, loops.0)?;
        let end = *at(&parts.loop_start, loops.1)?;
        out.get_mut(start as usize..end as usize)
            .ok_or(MeshFault::Shape)?
            .fill(face);
    }
    Ok(out)
}

/// The uses at the front of `rest` of one edge, taken off it.
pub(super) fn next_group<'a>(rest: &mut &'a [Use]) -> Option<&'a [Use]> {
    let head = rest.first()?;
    let same = rest.iter().take_while(|u| u.high == head.high).count();
    let (group, tail) = rest.split_at(same);
    *rest = tail;
    Some(group)
}

/// Finds every half-edge's opposite, and what kind of surface that makes.
fn pair(parts: &mut Parts, meter: &mut WorkMeter<'_>) -> Result<(), MeshError> {
    let tails = ends(parts)?;
    let (first, uses) = grouped(&parts.he_vert, &tails, parts.verts.len())?;
    let of_face = faces_of(parts)?;
    let mut pairs = alloc::vec![NONE; parts.he_vert.len()];
    let mut boundary = false;
    let mut general = false;
    for (low, bucket) in (0_u32..).zip(first.windows(2)) {
        let &[start, end] = bucket else {
            return Err(MeshFault::Shape.into());
        };
        let bucket = uses
            .get(start as usize..end as usize)
            .ok_or(MeshFault::Shape)?;
        let mut rest = bucket;
        while let Some(group) = next_group(&mut rest) {
            meter.charge(1)?;
            match *group {
                [_] => boundary = true,
                [a, b] => {
                    *pairs
                        .get_mut(a.half_edge as usize)
                        .ok_or(MeshFault::Shape)? = b.half_edge;
                    *pairs
                        .get_mut(b.half_edge as usize)
                        .ok_or(MeshFault::Shape)? = a.half_edge;
                }
                _ => {
                    general = true;
                    let high = group.first().ok_or(MeshFault::Shape)?.high;
                    let order = radial(parts, &of_face, group, low, high)?;
                    let ring = order.iter().cycle().skip(1);
                    for (&(h, against), &(next, next_against)) in order.iter().zip(ring) {
                        if against {
                            if next_against {
                                return Err(MeshFault::UnevenEdge { half_edge: h }.into());
                            }
                            *pairs.get_mut(h as usize).ok_or(MeshFault::Shape)? = next;
                            *pairs.get_mut(next as usize).ok_or(MeshFault::Shape)? = h;
                        }
                    }
                }
            }
        }
    }
    parts.he_pair = pairs;
    parts.kind = if general {
        MeshKind::General
    } else if boundary {
        MeshKind::Open
    } else {
        MeshKind::Closed
    };
    Ok(())
}

/// A plane's row and which way it looks, for the arrays of [`Parts`].
struct Rows<'a> {
    of_plane: &'a [u32],
    planes: &'a [Turned],
    /// Where `planes` start in `rows`.
    base: usize,
}

impl Rows<'_> {
    fn facing(&self, i: u32) -> Result<Facing, MeshFault> {
        let flipped = at(self.planes, i)?.flipped;
        let row = self
            .of_plane
            .get((i as usize).saturating_add(self.base))
            .copied()
            .ok_or(MeshFault::Shape)?;
        Ok(Facing { row, flipped })
    }
}

/// The faces in canonical order, as `keys` sorts them, written into the arrays of `parts`.
#[allow(
    clippy::too_many_arguments,
    reason = "the input, its corners, both sets of planes, the runs and the keys, each a separate \
              result of an earlier step of `build`"
)]
fn emit(
    parts: &mut Parts,
    input: &Polygons,
    corner_vertex: &[u32],
    faces: &Rows<'_>,
    edges: &Rows<'_>,
    sources: &[(NameId, Role, u32, u32)],
    keys: &[FaceKey],
) -> Result<(), MeshError> {
    parts.face_loops.push(0);
    parts.loop_start.push(0);
    let prop_of = |c: u32| input.corner_prop.get(c as usize).copied().unwrap_or(NONE);
    let mut open: Option<(u32, u32)> = None;
    for key in keys {
        parts.face_plane.push(faces.facing(key.face)?);
        parts.face_dev.push(
            input
                .face_dev
                .get(key.face as usize)
                .copied()
                .unwrap_or(Steps(0)),
        );
        let (first, last) = span(&input.face_loops, key.face)?;
        let mut outer = Some((first, key.start));
        let ordered = if last.saturating_sub(first) > 1 {
            outer = None;
            ordered_loops(input, corner_vertex, key)?
        } else {
            Vec::new()
        };
        for &(looped, from) in outer.iter().chain(&ordered) {
            let corners = span(&input.loop_start, looped)?;
            let mut corner = from;
            for _ in corners.0..corners.1 {
                parts.he_vert.push(*at(corner_vertex, corner)?);
                parts.he_plane.push(edges.facing(corner)?);
                parts.he_prop.push(prop_of(corner));
                corner = next_corner(corner, corners);
            }
            parts.loop_start.push(index(parts.he_vert.len())?);
        }
        parts
            .face_loops
            .push(index(parts.loop_start.len())?.saturating_sub(1));
        let done = index(parts.face_plane.len())?.saturating_sub(1);
        match open {
            Some((rank, _)) if rank == key.run => {}
            _ => {
                if let Some((rank, begin)) = open {
                    parts.runs.push(run_of(sources, rank, begin, done)?);
                }
                open = Some((key.run, done));
            }
        }
    }
    if let Some((rank, begin)) = open {
        let end = index(parts.face_plane.len())?;
        parts.runs.push(run_of(sources, rank, begin, end)?);
    }
    Ok(())
}

/// The run of the source ranked `rank`, over faces `begin..end`.
fn run_of(
    sources: &[(NameId, Role, u32, u32)],
    rank: u32,
    begin: u32,
    end: u32,
) -> Result<Run, MeshFault> {
    let &(source, role, flags, _) = at(sources, rank)?;
    Ok(Run {
        faces: begin..end,
        source,
        role,
        flags,
    })
}

/// The faces' keys, in canonical order.
fn sorted_faces(
    input: &Polygons,
    corner_vertex: &[u32],
    run_rank: &[u32],
    ids: &[u32],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<FaceKey>, MeshError> {
    let keys: Result<Vec<FaceKey>, RunError<MeshError>> =
        exec.try_map(meter, FACE_KEYS, PLAN, ids, |_, &face, m| {
            m.charge(1)?;
            Ok(face_key(input, corner_vertex, face, run_rank)?)
        });
    let keys = phase(keys)?;
    let first = |k: &FaceKey| (k.run, k.lowest, k.next);
    let order: Result<Vec<usize>, RunError<MeshError>> =
        exec.sort_stable(meter, SORT_FACES, SORT_PLAN, &keys, first);
    let order = phase(order)?;
    let mut sorted = Vec::with_capacity(keys.len());
    for i in order {
        sorted.push(*keys.get(i).ok_or(MeshFault::Shape)?);
    }
    Ok(sorted)
}

/// The mesh of `input`.
///
/// # Errors
///
/// [`MeshError::FacePlaneNeeded`] or [`MeshError::EdgePlaneNeeded`] for a face or edge off the grid
/// without its plane; [`MeshError::Fault`] for arrays that do not fit together or polygons the
/// checker refuses; the others as their names say.
pub fn build(
    input: &Polygons,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactMesh, MeshError> {
    let (faces, loops) = fits(input)?;
    let verts = vertices(input, meter)?;
    let face_ids: Vec<u32> = (0..faces).collect();
    let by_face: Result<Vec<Turned>, RunError<MeshError>> =
        exec.try_map(meter, FACE_PLANES, PLAN, &face_ids, |_, &face, m| {
            m.charge(1)?;
            face_plane(input, &verts, face)
        });
    let by_face = phase(by_face)?;
    let (face_of, after) = corner_links(input, faces)?;
    let corner_ids: Vec<u32> = (0..index(input.corners.len())?).collect();
    let by_edge: Result<Vec<Turned>, RunError<MeshError>> =
        exec.try_map(meter, EDGE_PLANES, PLAN, &corner_ids, |_, &corner, m| {
            m.charge(1)?;
            corner_plane(input, &verts, &by_face, (&face_of, &after), corner)
        });
    let by_edge = phase(by_edge)?;
    let mut own = Vec::with_capacity(verts.implicit.len().saturating_mul(3));
    for plane in verts.implicit.iter().flatten() {
        own.push(Turned::given(plane)?);
    }
    let (pooled, rows) = pool(&[&by_face, &by_edge, &own], exec, meter)?;
    let own_base = by_face.len().saturating_add(by_edge.len());

    let mut corner_vertex: Vec<u32> = Vec::with_capacity(input.corners.len());
    for &v in &input.corners {
        corner_vertex.push(*at(&verts.of_input, v)?);
    }
    let mut triples = Vec::new();
    if !verts.implicit.is_empty() {
        let row_of = |i: usize| rows.get(i).copied().ok_or(MeshFault::Shape);
        let mut corner_rows = Vec::with_capacity(input.corners.len());
        for (corner, &face) in face_of.iter().enumerate() {
            let edge_row = row_of(corner.saturating_add(by_face.len()))?;
            corner_rows.push((row_of(face as usize)?, edge_row));
        }
        let own_rows = rows.get(own_base..).ok_or(MeshFault::Shape)?;
        let place;
        (triples, place) = implicit_keys(&verts, &corner_vertex, &corner_rows, own_rows, &pooled)?;
        let base = index(verts.grid.len())?;
        for v in &mut corner_vertex {
            if let Some(i) = v.checked_sub(base) {
                *v = base.saturating_add(*at(&place, i)?);
            }
        }
    }

    let mut sources: Vec<(NameId, Role, u32, u32)> = Vec::with_capacity(input.runs.len());
    for (run, source) in (0_u32..).zip(&input.runs) {
        sources.push((source.name, source.role, source.flags, run));
    }
    sources.sort_unstable();
    let mut run_rank = alloc::vec![0_u32; input.runs.len()];
    for (rank, source) in (0_u32..).zip(&sources) {
        *run_rank
            .get_mut(source.3 as usize)
            .ok_or(MeshFault::Shape)? = rank;
    }
    let keys = sorted_faces(input, &corner_vertex, &run_rank, &face_ids, exec, meter)?;

    let mut parts = Parts {
        planes: pooled,
        verts: verts.grid.iter().map(|&p| Vertex::Grid(p)).collect(),
        face_plane: Vec::with_capacity(faces as usize),
        face_loops: Vec::with_capacity(input.face_loops.len()),
        loop_start: Vec::with_capacity((loops as usize).saturating_add(1)),
        he_vert: Vec::with_capacity(input.corners.len()),
        he_plane: Vec::with_capacity(input.corners.len()),
        he_pair: Vec::new(),
        he_prop: Vec::with_capacity(input.corners.len()),
        props: input.props.clone(),
        runs: Vec::new(),
        face_dev: Vec::with_capacity(faces as usize),
        class: input.class,
        kind: MeshKind::Open,
    };
    parts
        .verts
        .extend(triples.iter().map(|&t| Vertex::Planes(t)));
    let of_faces = Rows {
        of_plane: &rows,
        planes: &by_face,
        base: 0,
    };
    let of_edges = Rows {
        of_plane: &rows,
        planes: &by_edge,
        base: by_face.len(),
    };
    emit(
        &mut parts,
        input,
        &corner_vertex,
        &of_faces,
        &of_edges,
        &sources,
        &keys,
    )?;
    pair(&mut parts, meter)?;
    ExactMesh::new(parts, exec, meter)
}
