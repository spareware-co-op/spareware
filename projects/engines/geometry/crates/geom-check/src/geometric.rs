//! Group 2 of chapter 8.3, geometric consistency: every vertex lies on its faces' surfaces and at
//! its edges' curve ends, within the bound the body is recorded with, and every edge on its faces.
//!
//! What it does: [`check`] walks the faces, fins and edges in row order. A vertex lies on a flat
//! face's plane when its signed distance, computed exactly in integers, is at most the body's
//! bound; a vertex lies at a straight edge's end when a certified upper bound on its distance from
//! the curve's end point is at most the bound. An edge on a flat face follows from its two
//! vertices: a segment whose ends lie within b of a plane lies within b of it everywhere (distance
//! to a plane is affine along a segment), so no point of the edge is sampled. Every carrier this
//! group has no certified check for yet (a curved surface, a curve other than a line, a pcurve) is
//! reported as [`FaultKind::Unchecked`], never passed (8.3: "certified with bounds, not sampled").
//!
//! The bound: the body's [`Exactness`] record, in the steps of its coordinates (a vertex is a whole
//! number of grid steps, and the curves and surfaces are doubles in the same units, chapter 6.1):
//! 0 for `Exact`, the proven bound for `Certified` and the measured deviation for `Approximate`.
//!
//! Guarantees:
//!
//! - **Exact on flat faces.** A plane given by integer-valued doubles (its origin and two
//!   spanning vectors within ±(2^40 − 1)) becomes the integer plane through three grid points, and
//!   a vertex passes exactly when (n · p + d)^2 ≤ b^2 · |n|^2, compared in 320-bit integers; for
//!   b = 0 that is the orient test, n · p + d = 0. A plane not given by integers is unchecked.
//! - **Certified on lines.** A line's end points are enclosed with `geom_curve::tight`, which is
//!   exact when the doubles are, so a vertex exactly on the bound passes and one a step beyond
//!   fails.
//! - **Fails closed.** Every comparison whose operands cannot be formed (a value out of its range,
//!   an overflow) is a fault, never a pass.
//! - **No sampling, nothing allocated.** Each check is an exact predicate or one interval
//!   evaluation of a closed form.
//!
//! Method: chapter 8.3's group 2 and its certified-bounds rule
//! (`projects/engines/geometry/architecture/08-topology.md`), on the exact integer plane of 6.3
//! (`geom_num::plane`, after Campen and Kobbelt 2010, <https://doi.org/10.1111/j.1467-8659.2009.01609.x>)
//! and interval enclosures that stay exact where the doubles are (R. E. Moore, R. B. Kearfott and
//! M. J. Cloud, *Introduction to Interval Analysis*, SIAM 2009,
//! <https://doi.org/10.1137/1.9780898717716>).
//!
//! Cost: linear in faces, fins and edges, one meter unit each; constant work per row; nothing
//! allocated. Recursion: none.

use geom_curve::ParamCurve;
use geom_curve::tight;
use geom_num::interval::Interval;
use geom_num::plane::Plane as GridPlane;
use geom_num::wide::I320;
use geom_num::{Coord, Exactness, P3};
use geom_run::Stopped;
use geom_topo::entity::{Edge, Face, Fin, Vertex};
use geom_topo::ix::Arena;
use geom_topo::{Arenas, Body, CurveKind, Ix, SurfaceKind};

use crate::combinatorial::{fin_ends, fin_face, rows};
use crate::report::{Entity, Fault, FaultKind};
use crate::sink::Sink;

/// The largest grid coordinate, 2^40 − 1 steps (chapter 6.1).
const COORD_MAX: f64 = 1_099_511_627_775.0;

/// Group 2 on `body`, recorded `exactness`.
pub(crate) fn check(
    body: &Body,
    exactness: Exactness,
    sink: &mut Sink<'_, '_>,
) -> Result<(), Stopped> {
    let a = body.arenas();
    let b = bound(exactness);
    for f in rows::<Face>(a.rows(Arena::Face)) {
        sink.charge()?;
        if plane(a, f).is_none() {
            let surface = a
                .faces
                .surface
                .get(f)
                .copied()
                .map_or(Entity::Face(f), Entity::Surface);
            sink.push(Fault::at(FaultKind::Unchecked, surface).with(Entity::Face(f)));
        }
    }
    for n in rows::<Fin>(a.rows(Arena::Fin)) {
        sink.charge()?;
        if let Some(p) = a.fins.pcurve.get(n).copied().flatten() {
            sink.push(Fault::at(FaultKind::Unchecked, Entity::Pcurve(p)).with(Entity::Fin(n)));
        }
        let face = fin_face(a, n);
        let start = fin_ends(a, n).map(|(s, _)| s);
        if let (Some(face), Some(v)) = (face, start)
            && let Some(grid) = plane(a, face)
            && !on_plane(&grid, point(a, v), b)
        {
            sink.push(
                Fault::at(FaultKind::VertexOffSurface, Entity::Vertex(v)).with(Entity::Face(face)),
            );
        }
    }
    for e in rows::<Edge>(a.rows(Arena::Edge)) {
        sink.charge()?;
        let c = a.edges.curve.get(e).copied();
        if let Some(CurveKind::Line(line)) = c.and_then(|c| a.curves.get(c)) {
            let (lo, hi) = line.domain();
            let ends = [
                (a.edges.start.get(e).copied().flatten(), lo),
                (a.edges.end.get(e).copied().flatten(), hi),
            ];
            for (vertex, t) in ends {
                if let Some(v) = vertex
                    && !on_line(line.origin(), line.direction(), t, point(a, v), b)
                {
                    sink.push(
                        Fault::at(FaultKind::VertexOffCurve, Entity::Vertex(v))
                            .with(Entity::Edge(e)),
                    );
                }
            }
        } else {
            let curve = c.map_or(Entity::Edge(e), Entity::Curve);
            sink.push(Fault::at(FaultKind::Unchecked, curve).with(Entity::Edge(e)));
        }
    }
    Ok(())
}

/// The bound a vertex may lie from its carriers, in steps: 0 for an Exact body.
const fn bound(exactness: Exactness) -> u64 {
    match exactness {
        Exactness::Exact => 0,
        Exactness::Certified { bound, .. } => bound.get(),
        Exactness::Approximate { deviation, .. } => deviation.get(),
    }
}

/// The origin, the fallback of a vertex link no published body leaves dangling.
const ORIGIN: P3 = P3 {
    x: Coord::ZERO,
    y: Coord::ZERO,
    z: Coord::ZERO,
};

/// A vertex's point.
fn point(a: &Arenas, v: Ix<Vertex>) -> P3 {
    a.vertices
        .point
        .get(v)
        .and_then(|p| a.points.get(*p))
        .copied()
        .unwrap_or(ORIGIN)
}

/// The integer plane of a face whose surface is a plane given by integers: its origin and two
/// spanning vectors, every coordinate a whole number of steps within the grid's range.
fn plane(a: &Arenas, face: Ix<Face>) -> Option<GridPlane> {
    let surface = a.faces.surface.get(face).and_then(|s| a.surfaces.get(*s))?;
    let SurfaceKind::Plane(flat) = surface else {
        return None;
    };
    let (origin, along, across) = flat.vectors();
    let on_grid = |w: [f64; 3]| -> Option<P3> {
        let [px, py, pz] = w.map(grid);
        P3::new(px?, py?, pz?).ok()
    };
    let offset = |w: [f64; 3]| -> [f64; 3] {
        let [ox, oy, oz] = origin;
        let [wx, wy, wz] = w;
        [ox + wx, oy + wy, oz + wz]
    };
    GridPlane::through(
        on_grid(origin)?,
        on_grid(offset(along))?,
        on_grid(offset(across))?,
    )
    .ok()
}

/// A double that is a whole number of steps within the grid's range, as that number.
fn grid(x: f64) -> Option<i64> {
    if !(-COORD_MAX..=COORD_MAX).contains(&x) {
        return None;
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "|x| <= 2^40 - 1, so the integer part fits; the comparison below keeps x only if it is whole"
    )]
    let i = x as i64;
    #[allow(
        clippy::cast_precision_loss,
        reason = "|i| < 2^53, so the double holds it exactly"
    )]
    let back = i as f64;
    (back == x).then_some(i)
}

/// Whether `p` lies within `b` steps of the plane: (n · p + d)^2 ≤ b^2 · |n|^2, exactly.
fn on_plane(plane: &GridPlane, p: P3, b: u64) -> bool {
    let n = plane.n();
    let s = [(n[0], p.x), (n[1], p.y), (n[2], p.z)]
        .into_iter()
        .try_fold(plane.d(), |acc, (ni, c)| {
            ni.checked_mul(i128::from(c.get()))
                .and_then(|t| acc.checked_add(t))
        });
    let wide = |x: i128| I320::from_i128(x);
    let square = |x: &I320| x.checked_mul(x);
    let lhs = s.map(wide).as_ref().and_then(square);
    let norm = n.iter().try_fold(I320::from_i128(0), |acc, &ni| {
        square(&wide(ni)).and_then(|t| acc.checked_add(&t))
    });
    let rhs = square(&wide(i128::from(b)))
        .zip(norm)
        .and_then(|(b2, n2)| b2.checked_mul(&n2));
    lhs.zip(rhs).is_some_and(|(l, r)| l <= r)
}

/// Whether `vertex` lies within `limit_steps` of the line's point at parameter `param`,
/// origin + param · direction: the certified upper bound of the squared distance is at most the
/// certified lower bound of the squared limit. Nothing allocates.
fn on_line(
    origin: [f64; 3],
    direction: [f64; 3],
    param: f64,
    vertex: P3,
    limit_steps: u64,
) -> bool {
    let gap = |start: f64, step: f64, at: Coord| -> Option<Interval> {
        let along = tight::mul(tight::point(param).ok()?, tight::point(step).ok()?).ok()?;
        let on_line = tight::add(tight::point(start).ok()?, along).ok()?;
        #[allow(
            clippy::cast_precision_loss,
            reason = "a grid coordinate is below 2^40, so the double holds it exactly"
        )]
        let target = at.get() as f64;
        tight::square(tight::sub(on_line, tight::point(target).ok()?).ok()?).ok()
    };
    #[allow(
        clippy::cast_precision_loss,
        reason = "a bound past 2^53 steps exceeds every distance between points of the grid's range (below 2^42), so rounding it changes no answer"
    )]
    let limit = limit_steps as f64;
    let [start_x, start_y, start_z] = origin;
    let [step_x, step_y, step_z] = direction;
    let squared = gap(start_x, step_x, vertex.x)
        .zip(gap(start_y, step_y, vertex.y))
        .zip(gap(start_z, step_z, vertex.z))
        .and_then(|((gx, gy), gz)| tight::add(tight::add(gx, gy).ok()?, gz).ok());
    let allowed = tight::point(limit).and_then(tight::square).ok();
    squared
        .zip(allowed)
        .is_some_and(|(have, room)| have.hi() <= room.lo())
}
