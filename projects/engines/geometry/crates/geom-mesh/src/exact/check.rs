//! The mesh checker group: what every [`ExactMesh`](super::ExactMesh) has passed (19.2).
//!
//! What it does: [`check`] tests [`Parts`] for the rules of 19.2's "What it guarantees": every loop
//! closes, no edge has zero length, no face is degenerate or repeated, every vertex lies exactly on
//! its faces' and edges' planes, every edge is used equally often each way with consistent radial
//! pairing, and a `Closed` mesh has no boundary edge. It also holds the arrays to their canonical
//! order, so that a mesh's bytes depend only on its content.
//!
//! Guarantees:
//!
//! - **Exact.** Every test is on integers or exact predicates: a vertex is on a plane only when the
//!   plane's exact side of it is `Zero`.
//! - **Nothing read past its array.** The shape of the arrays is tested first, and every later test
//!   reads through checked lookups.
//! - **The first fault is named.** The tests run in a fixed order over the arrays in order, so one
//!   mesh gives one [`MeshFault`] on every target.
//! - **Off the grid too.** A face with a corner off the grid has its turn tested through its planes:
//!   the sum of its corners' cross products would need rationals, so each edge's two ends are
//!   ordered along the line of its two planes, and each loop's lowest corner, which a loop can only
//!   pass one way, says which way the loop goes round.
//!
//! Method: the closure certificate of csgrs's hypermesh
//! (<https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/src/surface_arrangement.rs>),
//! with exact radial order from 11.3, as 19.2 sets out
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`).
//!
//! Consulted: csgrs, MIT,
//!  <https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/src/surface_arrangement.rs>,
//!  for which rules a closure certificate holds; its method, not its code.
//!
//! Cost: linear in the half-edges, but for the sort of a bucket of edges at each vertex, and for
//! faces and three-plane points that sort when there are any such to compare. No recursion.

#![allow(
    clippy::many_single_char_names,
    reason = "a, b, c and d are points and n, u and v vectors, as in the formulas"
)]

use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_num::grid::{GridStep, Steps};
use geom_num::plane::{Plane, Tpi};
use geom_num::{Exactness, I256, I320, NumError, Sign, Wide};
use geom_poly::order::{Line, along, same_point};
use geom_run::{Exec, Phase, RunError, WorkMeter};

use super::build::{crossing, ends, faces_of, grouped, next_group, radial, span};
use super::{
    Facing, MeshError, MeshFault, MeshKind, NONE, PLAN, Parts, Vertex, at, gcd, morton, plane_key,
};

const CHECK_PLANES: Phase = Phase::new("exact mesh: check planes");
const CHECK_CORNERS: Phase = Phase::new("exact mesh: check corners");

/// Whether offsets start at 0, never fall, and end at `last`.
fn offsets(array: &[u32], last: usize) -> bool {
    array.first() == Some(&0)
        && array.windows(2).all(|p| p.first() <= p.last())
        && array.last().map(|&l| l as usize) == Some(last)
}

/// The arrays fit together, and every index is in range.
fn shape(p: &Parts) -> Result<(), MeshFault> {
    let (faces, halves) = (p.face_plane.len(), p.he_vert.len());
    let loops = p.loop_start.len().saturating_sub(1);
    let rows = p.props.rows();
    let fits = p.face_loops.len() == faces.saturating_add(1)
        && offsets(&p.face_loops, loops)
        && offsets(&p.loop_start, halves)
        && p.he_plane.len() == halves
        && p.he_pair.len() == halves
        && p.he_prop.len() == halves
        && p.face_dev.len() == faces
        && p.he_vert.iter().all(|&v| (v as usize) < p.verts.len())
        && p.he_pair
            .iter()
            .all(|&h| h == NONE || (h as usize) < halves)
        && p.he_prop.iter().all(|&r| r == NONE || (r as usize) < rows)
        && p.he_plane
            .iter()
            .chain(&p.face_plane)
            .all(|f| (f.row as usize) < p.planes.len())
        && p.face_loops.windows(2).all(|l| l.first() < l.last());
    if !fits {
        return Err(MeshFault::Shape);
    }
    let mut next = 0;
    for run in &p.runs {
        if run.faces.start != next || run.faces.end <= next {
            return Err(MeshFault::Shape);
        }
        next = run.faces.end;
    }
    if next as usize != faces {
        return Err(MeshFault::Shape);
    }
    Ok(())
}

/// Plane `row` is canonical, and sorts above the one before it.
fn plane_in_order(p: &Parts, row: u32, plane: &Plane) -> Result<(), MeshFault> {
    let key = plane_key(plane);
    let ([x, y, z], d) = key;
    let first = gcd(x.unsigned_abs(), y.unsigned_abs());
    let reduced = first == 1 || gcd(gcd(first, z.unsigned_abs()), d.unsigned_abs()) == 1;
    let upright = [x, y, z].iter().find(|c| **c != 0).is_some_and(|c| *c > 0);
    if !reduced || !upright {
        return Err(MeshFault::PlaneNotCanonical { row });
    }
    let before = row.checked_sub(1).map(|r| at(&p.planes, r)).transpose()?;
    if before.is_some_and(|b| plane_key(b) >= key) {
        return Err(MeshFault::PlanesOutOfOrder { row });
    }
    Ok(())
}

/// The pool is canonical and strictly rising.
fn pool(p: &Parts, exec: &Exec<'_>, meter: &mut WorkMeter<'_>) -> Result<(), MeshError> {
    let done: Result<Vec<()>, RunError<MeshError>> =
        exec.try_map(meter, CHECK_PLANES, PLAN, &p.planes, |i, plane, m| {
            m.charge(1)?;
            Ok(plane_in_order(p, super::index(i)?, plane)?)
        });
    done.map(|_| ()).map_err(MeshError::from)
}

/// A vertex's place in the order: grid points first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Grid(u128),
    Planes([u32; 3]),
}

/// The vertices are strictly rising, three-plane points are off the grid, and no two are one point.
fn vertices(p: &Parts, meter: &mut WorkMeter<'_>) -> Result<(), MeshError> {
    let mut last = None;
    let mut implicit: Vec<([i64; 3], u32, Tpi)> = Vec::new();
    for (&v, vertex) in p.verts.iter().zip(0_u32..) {
        meter.charge(1)?;
        let key = match v {
            Vertex::Grid(c) => Key::Grid(morton(c)),
            Vertex::Planes(rows) => {
                let [a, b, c] = rows;
                if a >= b || b >= c {
                    return Err(MeshFault::VerticesOutOfOrder { vertex }.into());
                }
                let [q, r, s] = rows.map(|row| at(&p.planes, row).copied());
                let tpi = Tpi::new(q?, r?, s?).map_err(|e| match e {
                    NumError::Degenerate => MeshError::Fault(MeshFault::PlanesDoNotMeet { vertex }),
                    other => other.into(),
                })?;
                let rounded = tpi.to_grid()?;
                if rounded.is_exact() {
                    return Err(MeshFault::ImplicitOnTheGrid { vertex }.into());
                }
                let at = rounded.value();
                implicit.push(([at.x.get(), at.y.get(), at.z.get()], vertex, tpi));
                Key::Planes(rows)
            }
        };
        if last.is_some_and(|l| l >= key) {
            return Err(MeshFault::VerticesOutOfOrder { vertex }.into());
        }
        last = Some(key);
    }
    // One point rounds one way, so repeats are neighbours among points that round alike.
    implicit.sort_unstable_by_key(|&(at, vertex, _)| (at, vertex));
    let mut rest = implicit.as_slice();
    while let Some((head, _)) = rest.split_first() {
        let same = rest.iter().take_while(|e| e.0 == head.0).count();
        let (group, tail) = rest.split_at(same);
        rest = tail;
        for (i, (_, first, a)) in group.iter().enumerate() {
            for (_, second, b) in group.iter().skip(i.saturating_add(1)) {
                meter.charge(1)?;
                if same_point(a, b)? {
                    return Err(MeshFault::RepeatedVertex {
                        first: *first,
                        second: *second,
                    }
                    .into());
                }
            }
        }
    }
    Ok(())
}

/// One face's vertices, by loop, for comparing faces.
fn sequence(p: &Parts, face: u32) -> Result<&[u32], MeshFault> {
    let (first, last) = span(&p.face_loops, face)?;
    let start = *at(&p.loop_start, first)?;
    let end = *at(&p.loop_start, last)?;
    p.he_vert
        .get(start as usize..end as usize)
        .ok_or(MeshFault::Shape)
}

/// How one face compares with another in canonical order.
fn compare(p: &Parts, a: u32, b: u32) -> Result<Ordering, MeshFault> {
    let (x, y) = (sequence(p, a)?, sequence(p, b)?);
    let (lx, ly) = (x.iter().min(), y.iter().min());
    let (la, lb) = (span(&p.face_loops, a)?, span(&p.face_loops, b)?);
    let loops = la.1.saturating_sub(la.0).cmp(&lb.1.saturating_sub(lb.0));
    Ok(lx.cmp(&ly).then(x.cmp(y)).then(loops))
}

/// Each loop starts at its lowest vertex, holes rise, and faces and runs are in order.
fn order(p: &Parts, meter: &mut WorkMeter<'_>) -> Result<(), MeshError> {
    for looped in 0..super::index(p.loop_start.len().saturating_sub(1))? {
        let (start, end) = span(&p.loop_start, looped)?;
        if end.saturating_sub(start) < 3 {
            return Err(MeshFault::ShortLoop { looped }.into());
        }
        let run = p
            .he_vert
            .get(start as usize..end as usize)
            .ok_or(MeshFault::Shape)?;
        if run.first() != run.iter().min() {
            return Err(MeshFault::LoopStart { looped }.into());
        }
    }
    for pair in p.runs.windows(2) {
        if let [a, b] = pair
            && (a.source, a.role) >= (b.source, b.role)
        {
            return Err(MeshFault::RunsOutOfOrder { run: b.faces.start }.into());
        }
    }
    for face in 0..super::index(p.face_plane.len())? {
        meter.charge(1)?;
        let (first, last) = span(&p.face_loops, face)?;
        let mut hole = None;
        for looped in first.saturating_add(1)..last {
            let start = *at(&p.he_vert, *at(&p.loop_start, looped)?)?;
            if hole.is_some_and(|h| h >= start) {
                return Err(MeshFault::HolesOutOfOrder { face }.into());
            }
            hole = Some(start);
        }
    }
    for run in &p.runs {
        for face in run.faces.start.saturating_add(1)..run.faces.end {
            let before = face.saturating_sub(1);
            match compare(p, before, face)? {
                Ordering::Less => {}
                Ordering::Equal => {
                    return Err(MeshFault::RepeatedFace {
                        first: before,
                        second: face,
                    }
                    .into());
                }
                Ordering::Greater => return Err(MeshFault::FacesOutOfOrder { face }.into()),
            }
        }
    }
    Ok(())
}

/// n · c + d. Within [`Plane::side`]'s budget every partial sum stays below 2^126, so wrapping
/// steps are exact, and this is the hottest test of the group.
fn value(plane: &Plane, c: geom_num::grid::P3) -> i128 {
    let [a, b, e] = plane.n();
    a.wrapping_mul(i128::from(c.x.get()))
        .wrapping_add(b.wrapping_mul(i128::from(c.y.get())))
        .wrapping_add(e.wrapping_mul(i128::from(c.z.get())))
        .wrapping_add(plane.d())
}

/// Whether a vertex of `p` is on `plane`.
#[inline]
fn on(p: &Parts, plane: &Plane, vertex: &Vertex) -> Result<bool, MeshError> {
    Ok(match vertex {
        Vertex::Grid(c) => value(plane, *c) == 0,
        Vertex::Planes(rows) => {
            let [a, b, c] = rows.map(|row| at(&p.planes, row).copied());
            plane.side_of_tpi(&Tpi::new(a?, b?, c?)?)? == Sign::Zero
        }
    })
}

/// A normal as it faces.
fn faced(plane: &Plane, facing: Facing) -> [i128; 3] {
    let n = plane.n();
    if facing.flipped {
        n.map(i128::saturating_neg)
    } else {
        n
    }
}

/// a × b's component `k` with `a`, `b` normals: its sign, in `i128` while that fits and exactly
/// through the wide path where it does not.
fn cross_sign(a: [i128; 3], b: [i128; 3], k: usize) -> Result<Sign, MeshError> {
    let pick = |v: [i128; 3], i: usize| v.get(i).copied().unwrap_or(0);
    let (i, j) = match k {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    };
    let (p, q, r, s) = (pick(a, i), pick(b, j), pick(a, j), pick(b, i));
    // Below 2^63 each, the products are below 2^126 and their difference fits.
    if let (Ok(w), Ok(x), Ok(y), Ok(z)) = (
        i64::try_from(p),
        i64::try_from(q),
        i64::try_from(r),
        i64::try_from(s),
    ) {
        let product = |m: i64, n: i64| i128::from(m).wrapping_mul(i128::from(n));
        return Ok(Sign::of_i128(product(w, x).wrapping_sub(product(y, z))));
    }
    let wide = |v: i128| geom_num::wide::I256::from_i128(v);
    let product = |m: i128, n: i128| wide(m).checked_mul(&wide(n));
    let difference = product(p, q)
        .zip(product(r, s))
        .and_then(|(x, y)| x.checked_sub(&y))
        .ok_or(NumError::BudgetExceeded)?;
    Ok(difference.signum())
}

/// The exact tests on one face: every corner on its planes, and the face turning its plane's way.
fn face_corners(p: &Parts, face: u32) -> Result<(), MeshError> {
    let facing = *at(&p.face_plane, face)?;
    let plane = at(&p.planes, facing.row)?;
    let normal = faced(plane, facing);
    let (first, last) = span(&p.face_loops, face)?;
    // The axis of the plane's first non-zero component decides which way a flat face turns.
    let axis = normal.iter().position(|c| *c != 0).unwrap_or(0);
    let (i, j) = match axis {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    };
    let mut area = 0_i128;
    let mut on_grid = true;
    for looped in first..last {
        let (start, end) = span(&p.loop_start, looped)?;
        let mut base = None;
        for h in start..end {
            let after = h.saturating_add(1);
            let next = if after == end { start } else { after };
            let (u, v) = (*at(&p.he_vert, h)?, *at(&p.he_vert, next)?);
            if u == v {
                return Err(MeshFault::ZeroEdge { half_edge: h }.into());
            }
            let (a, b) = (at(&p.verts, u)?, at(&p.verts, v)?);
            if !on(p, plane, a)? {
                return Err(MeshFault::OffFacePlane { half_edge: h }.into());
            }
            let edge = *at(&p.he_plane, h)?;
            let through = at(&p.planes, edge.row)?;
            if !on(p, through, a)? || !on(p, through, b)? {
                return Err(MeshFault::OffEdgePlane { half_edge: h }.into());
            }
            if edge.row == facing.row {
                return Err(MeshFault::EdgePlaneFacing { half_edge: h }.into());
            }
            let (&Vertex::Grid(c), &Vertex::Grid(d)) = (a, b) else {
                on_grid = false;
                runs_its_way(p, (facing, edge), (u, v), h)?;
                continue;
            };
            let run = [
                coordinate(d, 0).wrapping_sub(coordinate(c, 0)),
                coordinate(d, 1).wrapping_sub(coordinate(c, 1)),
                coordinate(d, 2).wrapping_sub(coordinate(c, 2)),
            ];
            // m · (n × u) = (m × n) · u, and m × n is parallel to u.
            let k = run.iter().position(|c| *c != 0).unwrap_or(0);
            let u_k = run.get(k).copied().unwrap_or(0);
            if cross_sign(faced(through, edge), normal, k)? != Sign::of_i128(u_k) {
                return Err(MeshFault::EdgePlaneFacing { half_edge: h }.into());
            }
            let (bx, by) = *base.get_or_insert((coordinate(c, i), coordinate(c, j)));
            let (x0, y0) = (
                coordinate(c, i).wrapping_sub(bx),
                coordinate(c, j).wrapping_sub(by),
            );
            let (x1, y1) = (
                coordinate(d, i).wrapping_sub(bx),
                coordinate(d, j).wrapping_sub(by),
            );
            // Each term is below 2^83, and 2^32 corners keep the sum below 2^115.
            area = area.wrapping_add(x0.wrapping_mul(y1).wrapping_sub(x1.wrapping_mul(y0)));
        }
    }
    if !on_grid {
        for looped in first..last {
            turns_its_way(p, looped, looped == first, face)?;
        }
    }
    if on_grid {
        let want = Sign::of_i128(normal.get(axis).copied().unwrap_or(0));
        let got = Sign::of_i128(area);
        if got == Sign::Zero {
            return Err(MeshFault::DegenerateFace { face }.into());
        }
        if got != want {
            return Err(MeshFault::WrongWayRound { face }.into());
        }
    }
    Ok(())
}

/// [`face_corners`] on every face.
fn corners(p: &Parts, exec: &Exec<'_>, meter: &mut WorkMeter<'_>) -> Result<(), MeshError> {
    let ids: Vec<u32> = (0..super::index(p.face_plane.len())?).collect();
    let done: Result<Vec<()>, RunError<MeshError>> =
        exec.try_map(meter, CHECK_CORNERS, PLAN, &ids, |_, &face, m| {
            m.charge(1)?;
            face_corners(p, face)
        });
    done.map(|_| ()).map_err(MeshError::from)
}

/// Coordinate `k` of a grid point.
fn coordinate(p: geom_num::grid::P3, k: usize) -> i128 {
    i128::from(match k {
        0 => p.x.get(),
        1 => p.y.get(),
        _ => p.z.get(),
    })
}

/// `facing`'s plane, turned the way it faces.
fn turned(p: &Parts, facing: Facing) -> Result<Plane, MeshError> {
    let plane = at(&p.planes, facing.row)?;
    if facing.flipped {
        Ok(Plane::new(
            plane.n().map(i128::wrapping_neg),
            plane.d().wrapping_neg(),
        )?)
    } else {
        Ok(*plane)
    }
}

/// The half-edge `h`, from `u` to `v`, runs along m × n for its plane's normal m and its face's n,
/// so `v` comes first on the line of the two planes, which runs along n × m.
fn runs_its_way(
    p: &Parts,
    (face, edge): (Facing, Facing),
    (u, v): (u32, u32),
    h: u32,
) -> Result<(), MeshError> {
    let line = Line::new(turned(p, face)?, turned(p, edge)?)?;
    let order = along(&line, &crossing(p, &line, u)?, &crossing(p, &line, v)?)?;
    if order == Ordering::Greater {
        Ok(())
    } else {
        Err(MeshFault::EdgePlaneFacing { half_edge: h }.into())
    }
}

/// A vertex's coordinates over a positive weight.
fn place(p: &Parts, v: u32) -> Result<([I320; 3], I256), MeshError> {
    match *at(&p.verts, v)? {
        Vertex::Grid(c) => {
            let part = |k: i64| I320::from_i64(k);
            let at_grid = [part(c.x.get()), part(c.y.get()), part(c.z.get())];
            Ok((at_grid, I256::from_i64(1)))
        }
        Vertex::Planes([a, b, c]) => {
            let row = |r: u32| at(&p.planes, r).copied();
            let (x, w) = Tpi::new(row(a)?, row(b)?, row(c)?)?.homogeneous()?;
            if !w.is_negative() {
                return Ok((x, w));
            }
            let [i, j, k] = x;
            let flip = |v: &I320| v.checked_neg().ok_or(NumError::BudgetExceeded);
            let w = w.checked_neg().ok_or(NumError::BudgetExceeded)?;
            Ok(([flip(&i)?, flip(&j)?, flip(&k)?], w))
        }
    }
}

/// Whether `a` sorts before `b` by x, then y, then z: at most 2^294 · 2^252, in 576 bits.
fn lower(a: &([I320; 3], I256), b: &([I320; 3], I256)) -> Result<bool, NumError> {
    let (x, w) = a;
    let (y, v) = b;
    let wide =
        |c: &I320, d: &I256| -> Option<Wide<9>> { c.resize::<9>()?.checked_mul(&d.resize::<9>()?) };
    for (c, d) in x.iter().zip(y) {
        let gap = wide(c, v)
            .zip(wide(d, w))
            .and_then(|(left, right)| left.checked_sub(&right))
            .ok_or(NumError::BudgetExceeded)?;
        match gap.signum() {
            Sign::Negative => return Ok(true),
            Sign::Positive => return Ok(false),
            Sign::Zero => {}
        }
    }
    Ok(false)
}

/// Which side of `plane` vertex `v` is on.
fn side(p: &Parts, plane: &Plane, v: u32) -> Result<Sign, MeshError> {
    Ok(match *at(&p.verts, v)? {
        Vertex::Grid(c) => plane.side(c)?,
        Vertex::Planes([a, b, c]) => {
            let row = |r: u32| at(&p.planes, r).copied();
            plane.side_of_tpi(&Tpi::new(row(a)?, row(b)?, row(c)?)?)?
        }
    })
}

/// Loop `looped` of `face` goes round the way it should. Its lowest corner is an extreme of the
/// loop, so a loop round the face turns left there, and a hole right: the corner before is on the
/// inner side of the next edge's plane, or the outer side. A loop that touches itself at that corner
/// passes it more than once, turning one way only at the pass that goes round the outside.
fn turns_its_way(p: &Parts, looped: u32, outer: bool, face: u32) -> Result<(), MeshError> {
    let (start, end) = span(&p.loop_start, looped)?;
    let mut lowest = None;
    for h in start..end {
        let v = *at(&p.he_vert, h)?;
        let here = place(p, v)?;
        let better = match &lowest {
            Some((_, held)) => lower(&here, held)?,
            None => true,
        };
        if better {
            lowest = Some((v, here));
        }
    }
    let (corner, _) = lowest.ok_or(MeshFault::Shape)?;
    let want = if outer {
        Sign::Positive
    } else {
        Sign::Negative
    };
    let mut flat = true;
    for h in start..end {
        if *at(&p.he_vert, h)? != corner {
            continue;
        }
        let before = if h == start {
            end.saturating_sub(1)
        } else {
            h.saturating_sub(1)
        };
        let edge = turned(p, *at(&p.he_plane, h)?)?;
        let turn = side(p, &edge, *at(&p.he_vert, before)?)?;
        if turn == want {
            return Ok(());
        }
        flat &= turn == Sign::Zero;
    }
    if flat {
        Err(MeshFault::DegenerateFace { face }.into())
    } else {
        Err(MeshFault::WrongWayRound { face }.into())
    }
}

/// Every half-edge's pair is its opposite, every edge is used evenly, an edge of more than two is
/// paired in radial order, and the kind is what the edges make it.
fn pairs(p: &Parts, meter: &mut WorkMeter<'_>) -> Result<(), MeshError> {
    let tails = ends(p)?;
    let (first, uses) = grouped(&p.he_vert, &tails, p.verts.len())?;
    let of_face = faces_of(p)?;
    let (mut boundary, mut general) = (false, false);
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
            let head = *group.first().ok_or(MeshFault::Shape)?;
            let half_edge = head.half_edge;
            let forward = group.iter().filter(|u| u.forward).count();
            if group.len() == 1 {
                if *at(&p.he_pair, half_edge)? != NONE {
                    return Err(MeshFault::BadPair { half_edge }.into());
                }
                boundary = true;
                continue;
            }
            if forward.saturating_mul(2) != group.len() {
                return Err(MeshFault::UnevenEdge { half_edge }.into());
            }
            if let [a, b] = *group {
                let (x, y) = (a.half_edge, b.half_edge);
                if *at(&p.he_pair, x)? != y || *at(&p.he_pair, y)? != x {
                    return Err(MeshFault::BadPair { half_edge: x }.into());
                }
                continue;
            }
            general = true;
            let order = radial(p, &of_face, group, low, head.high)?;
            let ring = order.iter().cycle().skip(1);
            for (&(h, against), &(next, next_against)) in order.iter().zip(ring) {
                if against
                    && (next_against || *at(&p.he_pair, h)? != next || *at(&p.he_pair, next)? != h)
                {
                    return Err(MeshFault::RadialOrder { half_edge: h }.into());
                }
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
    if p.kind != kind {
        return Err(MeshFault::WrongKind.into());
    }
    Ok(())
}

/// No face is further from its source than the class says: its bound, or nothing at all if exact.
fn deviations(p: &Parts) -> Result<(), MeshError> {
    let limit: Option<(Steps, GridStep)> = match p.class {
        Exactness::Exact => None,
        Exactness::Certified { bound, step } => Some((bound, step)),
        Exactness::Approximate { deviation, step } => Some((deviation, step)),
    };
    for (&dev, face) in p.face_dev.iter().zip(0_u32..) {
        if limit.map_or(dev.0 != 0, |(bound, _)| dev.0 > bound.0) {
            return Err(MeshFault::Deviation { face }.into());
        }
    }
    Ok(())
}

/// Every rule of the mesh checker group, in a fixed order.
///
/// # Errors
///
/// [`MeshError::Fault`] naming the first rule broken and where; [`MeshError::Num`] for a fault of
/// the numbers layer, and [`MeshError::Stopped`] when the budget runs out.
pub fn check(p: &Parts, exec: &Exec<'_>, meter: &mut WorkMeter<'_>) -> Result<(), MeshError> {
    shape(p)?;
    pool(p, exec, meter)?;
    vertices(p, meter)?;
    order(p, meter)?;
    corners(p, exec, meter)?;
    pairs(p, meter)?;
    deviations(p)
}
