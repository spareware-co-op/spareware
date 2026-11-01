//! Where a point is against a closed exact mesh: on its surface, or by its winding number (19.11).
//!
//! What it does: [`Classifier`] holds a mesh and a tree of its faces' boxes in plan. [`Classifier::
//! locate`] casts a ray up the z axis from a grid point and adds up its signed crossings: the
//! winding number, 1 inside a solid, 0 outside. [`Classifier::locate_all`] does that for many
//! points on the parallel helper. [`Classifier::beside`] takes a three-plane point, whose ray would
//! not be a line of the grid, and traces from a grid corner near it with `geom-poly`'s exact
//! winding tracing.
//!
//! Guarantees:
//!
//! - **Exact.** Every decision is an integer sign or an exact predicate, within 6.2's budget.
//! - **Only the start moves.** A ray through an edge or a vertex in plan is settled by moving its
//!   start by an infinitesimal, (ε², ε) in x and y, never by moving the mesh: so a mesh with
//!   three-plane vertices needs no perturbation of its own, and one query has one answer. A point
//!   on the surface is told first, unmoved.
//! - **Closed meshes only.** Inside and outside are undefined for an open surface, which is refused.
//! - **The threads do not show.** [`Classifier::locate_all`] gives the same list on any pool.
//!
//! Method: ray crossing with Simulation of Simplicity on the start point only (19.11, 6.2; the
//! evidence is `projects/kit/notes/G01-evidence/05-exact-arithmetic-determinism.md`, section 5);
//! the signed count is Jacobson, Kavan and Sorkine-Hornung's winding number,
//! <https://doi.org/10.1145/2461912.2461916>, in the exact form of 11.2. In a face, a second ray
//! along x counts edges by a half-open rule on y, which is the same ε. Three-plane corners are
//! compared by the order of points along a line of two planes, 11.3's `along`.
//!
//! Cost: a query walks the tree for the faces over a point in plan, and each edge of those faces
//! once: a few signs for an edge between grid points, three determinants for one off the grid.
//! [`Classifier::beside`] splits the faces near the point into convex pieces and traces once.

#![allow(
    clippy::many_single_char_names,
    reason = "the names of the geometry: points, the axes i, j and k, and a line's two ends"
)]

use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_bvh::{Aabb, BoxTree, Entry};
use geom_num::distance::product_difference;
use geom_num::grid::P3;
use geom_num::plane::{Plane, Tpi};
use geom_num::{NumError, Sign};
use geom_poly::convex::{Piece, convex_pieces};
use geom_poly::order::{Line, along};
use geom_poly::trace::{Cell, Polygon, TraceError, classify};
use geom_poly::winding::Winding;
use geom_run::{Exec, Phase, RunError, WorkMeter};

use crate::exact::build::span;
use crate::exact::{ExactMesh, Facing, MeshError, MeshFault, MeshKind, PLAN, Parts, Vertex, at};

const LOCATE: Phase = Phase::new("classify: points");

/// How many corners [`Classifier::beside`] tries for one off the surface: the diagonal from a point
/// leaves any face after one step, unless it runs along it, and no mesh runs along all of these.
const CORNERS: i64 = 64;

/// Where a point is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Located {
    /// Exactly on the surface.
    OnSurface,
    /// Off the surface, with this winding number: 1 inside a solid, 0 outside, 2 where two overlap.
    Winding(i64),
}

/// One edge's part in a ray through a face.
#[derive(Clone, Copy, Default)]
struct Step {
    /// The moved ray crosses it.
    crossed: bool,
    /// The unmoved point is on it.
    on: bool,
}

/// How a point stands in a face, seen along an axis.
#[derive(Clone, Copy, Default)]
struct InFace {
    /// The moved point is inside: an odd number of edges to its right.
    inside: bool,
    /// The point is exactly on an edge.
    boundary: bool,
}

/// Coordinate `k` of a point.
fn coordinate(p: P3, k: usize) -> i128 {
    i128::from(match k {
        0 => p.x.get(),
        1 => p.y.get(),
        _ => p.z.get(),
    })
}

/// The two axes after `k`, in cyclic order.
const fn other_axes(k: usize) -> (usize, usize) {
    match k {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    }
}

/// The plane where coordinate `k` is `value`.
fn axis_plane(k: usize, value: i128) -> Result<Plane, NumError> {
    let mut n = [0; 3];
    if let Some(slot) = n.get_mut(k) {
        *slot = 1;
    }
    Plane::new(n, value.wrapping_neg())
}

/// A plane's normal as it faces.
fn faced(plane: &Plane, facing: Facing) -> [i128; 3] {
    if facing.flipped {
        plane.n().map(i128::wrapping_neg)
    } else {
        plane.n()
    }
}

/// A plane turned to look as it faces.
fn turned(plane: &Plane, facing: Facing) -> Result<Plane, NumError> {
    if facing.flipped {
        Plane::new(plane.n().map(i128::wrapping_neg), plane.d().wrapping_neg())
    } else {
        Ok(*plane)
    }
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

/// The edge from `u` to `v` of a polygon on grid points, in plan coordinates, against point `p`
/// moved by (ε², ε). Differences are below 2^41, so every product fits and wrapping is exact.
fn grid_edge(u: [i128; 2], v: [i128; 2], p: [i128; 2]) -> Step {
    let ([ui, uj], [vi, vj], [pi, pj]) = (u, v, p);
    let at_point = (ui == pi && uj == pj) || (vi == pi && vj == pj);
    if (uj <= pj) == (vj <= pj) {
        let level = uj == pj && vj == pj && ui.min(vi) <= pi && pi <= ui.max(vi);
        return Step {
            crossed: false,
            on: at_point || level,
        };
    }
    // Lower end to upper end, so the sign below is that of the crossing's x less the point's.
    let (ai, aj, bi, bj) = if uj < vj {
        (ui, uj, vi, vj)
    } else {
        (vi, vj, ui, uj)
    };
    let (run, rise) = (bi.wrapping_sub(ai), bj.wrapping_sub(aj));
    let right = ai
        .wrapping_sub(pi)
        .wrapping_mul(rise)
        .wrapping_add(pj.wrapping_sub(aj).wrapping_mul(run));
    match Sign::of_i128(right) {
        Sign::Positive => Step {
            crossed: true,
            on: false,
        },
        Sign::Negative => Step::default(),
        // On the edge: at height ε the edge has moved ε·run/rise along x, and the point ε².
        Sign::Zero => Step {
            crossed: run > 0,
            on: true,
        },
    }
}

/// A closed mesh ready for point queries.
#[derive(Debug)]
pub struct Classifier<'m> {
    mesh: &'m ExactMesh,
    tree: BoxTree<2>,
    /// The three-plane vertices, in order, after the grid ones.
    implicit: Vec<Tpi>,
    /// The first three-plane vertex.
    first_implicit: usize,
}

impl<'m> Classifier<'m> {
    /// A classifier of `mesh`.
    ///
    /// # Errors
    ///
    /// [`MeshError::NotClosed`] for an open mesh; the others as their names say.
    pub fn new(
        mesh: &'m ExactMesh,
        exec: &Exec<'_>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Self, MeshError> {
        if mesh.kind() == MeshKind::Open {
            return Err(MeshError::NotClosed);
        }
        let p = mesh.parts();
        let first_implicit = p
            .verts
            .iter()
            .position(|v| matches!(v, Vertex::Planes(_)))
            .unwrap_or(p.verts.len());
        let mut implicit = Vec::new();
        for v in p.verts.iter().skip(first_implicit) {
            let Vertex::Planes(rows) = *v else {
                return Err(MeshFault::Shape.into());
            };
            let [a, b, c] = rows.map(|row| at(&p.planes, row).copied());
            implicit.push(Tpi::meeting(a?, b?, c?)?);
        }
        let mut reach = Vec::with_capacity(p.verts.len());
        for (i, v) in p.verts.iter().enumerate() {
            let (c, pad) = match *v {
                Vertex::Grid(c) => (c, 0),
                Vertex::Planes(_) => {
                    let j = i.saturating_sub(first_implicit);
                    let t = implicit.get(j).ok_or(MeshFault::Shape)?;
                    (t.to_grid()?.value(), 1)
                }
            };
            reach.push(([c.x.get(), c.y.get(), c.z.get()], pad));
        }
        let mut entries = Vec::with_capacity(mesh.faces());
        for (face, key) in (0..crate::exact::index(mesh.faces())?).zip(0_u64..) {
            meter.charge(1)?;
            let (first, last) = span(&p.face_loops, face)?;
            let (start, end) = (*at(&p.loop_start, first)?, *at(&p.loop_start, last)?);
            let mut low = [i64::MAX; 3];
            let mut high = [i64::MIN; 3];
            for h in start..end {
                let &(c, pad) = at(&reach, *at(&p.he_vert, h)?)?;
                for ((l, u), x) in low.iter_mut().zip(&mut high).zip(c) {
                    *l = (*l).min(x.saturating_sub(pad));
                    *u = (*u).max(x.saturating_add(pad));
                }
            }
            let [lx, ly, _] = low;
            let [ux, uy, _] = high;
            entries.push(Entry {
                key,
                bbox: Aabb::new([lx, ly], [ux, uy])?,
            });
        }
        let tree = BoxTree::build(&entries, exec, meter)?;
        Ok(Self {
            mesh,
            tree,
            implicit,
            first_implicit,
        })
    }

    /// Vertex `v`'s grid point, if it has one.
    fn grid(&self, v: u32) -> Result<Option<P3>, MeshFault> {
        Ok(match *at(&self.mesh.parts().verts, v)? {
            Vertex::Grid(c) => Some(c),
            Vertex::Planes(_) => None,
        })
    }

    /// Which side of `plane` vertex `v` is on.
    fn side(&self, plane: &Plane, v: u32) -> Result<Sign, MeshError> {
        if let Some(c) = self.grid(v)? {
            return Ok(plane.side(c)?);
        }
        let i = (v as usize).saturating_sub(self.first_implicit);
        let t = self.implicit.get(i).ok_or(MeshFault::Shape)?;
        Ok(plane.side_of_tpi(t)?)
    }

    /// The edge `h`, from `u` to `v`, of `face`, with an end off the grid, against point `p`
    /// moved by (ε², ε) in the plan of axes `i` and `j`, whose third is `k`.
    fn plane_edge(
        &self,
        (face, h): (u32, u32),
        (u, v): (u32, u32),
        p: [i128; 2],
        (i, j, k): (usize, usize, usize),
    ) -> Result<Step, MeshError> {
        let parts = self.mesh.parts();
        let [pi, pj] = p;
        let level = axis_plane(j, pj)?;
        let (above_u, above_v) = (self.side(&level, u)?, self.side(&level, v)?);
        let across = axis_plane(i, pi)?;
        if (above_u == Sign::Positive) == (above_v == Sign::Positive) {
            // No crossing. The point is on the edge only if an end is at its level.
            let (au, av) = (self.side(&across, u)?, self.side(&across, v)?);
            let level_edge = above_u == Sign::Zero && above_v == Sign::Zero && au != av;
            let at_end = (above_u == Sign::Zero && au == Sign::Zero)
                || (above_v == Sign::Zero && av == Sign::Zero);
            return Ok(Step {
                crossed: false,
                on: level_edge || at_end,
            });
        }
        let facing = *at(&parts.face_plane, face)?;
        let edge = *at(&parts.he_plane, h)?;
        let (surface, through) = (
            *at(&parts.planes, facing.row)?,
            *at(&parts.planes, edge.row)?,
        );
        let line = Line::new(surface, level)?;
        // The line runs along n × e_j, whose component on axis i is −n_k.
        let rising = surface.n().get(k).copied().unwrap_or(0).is_negative();
        Ok(match along(&line, &across, &through)? {
            Ordering::Less => Step {
                crossed: rising,
                on: false,
            },
            Ordering::Greater => Step {
                crossed: !rising,
                on: false,
            },
            Ordering::Equal => {
                // As in `grid_edge`: the edge's lean, the sign of its dx/dy, from n × m.
                let (n, m) = (surface.n(), through.n());
                let pick = |w: [i128; 3], a: usize| w.get(a).copied().unwrap_or(0);
                let run = product_difference(pick(n, j), pick(m, k), pick(n, k), pick(m, j))?;
                let rise = product_difference(pick(n, k), pick(m, i), pick(n, i), pick(m, k))?;
                Step {
                    crossed: run.mul(rise) == Sign::Positive,
                    on: true,
                }
            }
        })
    }

    /// How `p` stands in `face` seen along axis `k`, on which the face's normal is not zero.
    fn in_face(&self, face: u32, p: P3, k: usize) -> Result<InFace, MeshError> {
        let parts = self.mesh.parts();
        let (i, j) = other_axes(k);
        let plan = [coordinate(p, i), coordinate(p, j)];
        let (first, last) = span(&parts.face_loops, face)?;
        let mut out = InFace::default();
        for looped in first..last {
            let (start, end) = span(&parts.loop_start, looped)?;
            for h in start..end {
                let after = h.saturating_add(1);
                let next = if after == end { start } else { after };
                let (u, v) = (*at(&parts.he_vert, h)?, *at(&parts.he_vert, next)?);
                let step = match (self.grid(u)?, self.grid(v)?) {
                    (Some(a), Some(b)) => grid_edge(
                        [coordinate(a, i), coordinate(a, j)],
                        [coordinate(b, i), coordinate(b, j)],
                        plan,
                    ),
                    _ => self.plane_edge((face, h), (u, v), plan, (i, j, k))?,
                };
                out.inside ^= step.crossed;
                out.boundary |= step.on;
            }
        }
        Ok(out)
    }

    /// Where grid point `p` is.
    ///
    /// # Errors
    ///
    /// [`MeshError::Stopped`] and the tree's refusals; the others, which a checked mesh cannot
    /// reach, as their names say.
    pub fn locate(&self, p: P3, meter: &mut WorkMeter<'_>) -> Result<Located, MeshError> {
        let parts = self.mesh.parts();
        let plan = [p.x.get(), p.y.get()];
        let mut winding = 0_i64;
        for key in self.tree.overlapping(&Aabb::new(plan, plan)?, meter)? {
            meter.charge(1)?;
            let face = u32::try_from(key).map_err(|_| MeshError::Full)?;
            let facing = *at(&parts.face_plane, face)?;
            let plane = at(&parts.planes, facing.row)?;
            let normal = faced(plane, facing);
            let side = plane.side(p)?;
            let side = if facing.flipped { side.negate() } else { side };
            if side == Sign::Zero {
                let seen = self.in_face(face, p, dominant(normal))?;
                if seen.inside || seen.boundary {
                    return Ok(Located::OnSurface);
                }
                continue;
            }
            let up = Sign::of_i128(normal.get(2).copied().unwrap_or(0));
            // The ray meets the plane above p where p is on the side the normal's z leaves.
            if up == Sign::Zero || up == side {
                continue;
            }
            if self.in_face(face, p, 2)?.inside {
                winding = winding.saturating_add(i64::from(up.as_i8()));
            }
        }
        Ok(Located::Winding(winding))
    }

    /// Where each of `points` is, in order.
    ///
    /// # Errors
    ///
    /// As for [`Self::locate`].
    pub fn locate_all(
        &self,
        points: &[P3],
        exec: &Exec<'_>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<Located>, MeshError> {
        let found: Result<Vec<Located>, RunError<MeshError>> =
            exec.try_map(meter, LOCATE, PLAN, points, |_, p, m| self.locate(*p, m));
        found.map_err(MeshError::from)
    }

    /// A grid corner near `c`, off the surface, with its winding number, and the far corner of the
    /// box they span around `c`.
    fn corner_near(
        &self,
        c: P3,
        meter: &mut WorkMeter<'_>,
    ) -> Result<([i64; 3], [i64; 3], i64), MeshError> {
        let at_c = [c.x.get(), c.y.get(), c.z.get()];
        for reach in 1..=CORNERS {
            let [x, y, z] = at_c.map(|v| v.saturating_sub(reach));
            let Ok(corner) = P3::new(x, y, z) else {
                continue;
            };
            if let Located::Winding(w) = self.locate(corner, meter)? {
                return Ok(([x, y, z], at_c.map(|v| v.saturating_add(reach)), w));
            }
        }
        Err(MeshError::Defect)
    }

    /// Every convex piece of the faces that reach the box from `low` to `high`, as polygons of
    /// entry 0.
    fn polygons(
        &self,
        low: [i64; 3],
        high: [i64; 3],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<Polygon>, MeshError> {
        let parts = self.mesh.parts();
        let [lx, ly, _] = low;
        let [ux, uy, _] = high;
        let mut out = Vec::new();
        for key in self
            .tree
            .overlapping(&Aabb::new([lx, ly], [ux, uy])?, meter)?
        {
            let face = u32::try_from(key).map_err(|_| MeshError::Full)?;
            let pieces = pieces_of(parts, face, meter)?;
            out.extend(pieces.into_iter().map(|piece| Polygon {
                piece,
                entry: Some(0),
            }));
        }
        Ok(out)
    }

    /// The winding number just beside three-plane point `q`, on the side its first plane faces. Off
    /// the surface that is its winding number. On the surface it is one side's.
    ///
    /// # Errors
    ///
    /// [`MeshError::Defect`] if no grid corner near `q` is off the surface; the others as for
    /// [`Self::locate`], and as tracing gives them.
    pub fn beside(&self, q: &Tpi, meter: &mut WorkMeter<'_>) -> Result<i64, MeshError> {
        let (low, high, base) = self.corner_near(q.to_grid()?.value(), meter)?;
        let mut polygons = self.polygons(low, high, meter)?;
        let [p, a, b] = *q.planes();
        let corner = Piece {
            face: p,
            bounds: alloc::vec![a, b],
        };
        polygons.push(Polygon {
            piece: corner.clone(),
            entry: None,
        });
        let mut pieces: Vec<Vec<Piece>> = alloc::vec![Vec::new(); polygons.len()];
        if let Some(last) = pieces.last_mut() {
            last.push(corner);
        }
        let mut reference = Winding::default();
        let start = i32::try_from(base).map_err(|_| NumError::BudgetExceeded)?;
        reference.add(0, start).map_err(|_| MeshError::Defect)?;
        // A box is in half steps.
        let cell = Cell {
            low: low.map(|v| v.saturating_mul(2)),
            high: high.map(|v| v.saturating_mul(2)),
        };
        let traced = classify(&cell, &reference, &polygons, &pieces, meter).map_err(trace_error)?;
        let one = traced.first().ok_or(MeshError::Defect)?;
        Ok(i64::from(one.front.count(0)))
    }
}

/// The convex pieces of face `face`, facing out.
pub(crate) fn pieces_of(
    parts: &Parts,
    face: u32,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Piece>, MeshError> {
    let facing = *at(&parts.face_plane, face)?;
    let surface = turned(at(&parts.planes, facing.row)?, facing)?;
    let (first, last) = span(&parts.face_loops, face)?;
    let mut rings = Vec::new();
    for looped in first..last {
        let (start, end) = span(&parts.loop_start, looped)?;
        let mut ring: Vec<Plane> = Vec::new();
        for h in start..end {
            let edge = *at(&parts.he_plane, h)?;
            let side = turned(at(&parts.planes, edge.row)?, edge)?;
            // Consecutive vertices on one straight edge share a plane: one side.
            if ring.last() != Some(&side) {
                ring.push(side);
            }
        }
        if ring.first() == ring.last() {
            ring.pop();
        }
        rings.push(ring);
    }
    convex_pieces(&surface, &rings, meter).map_err(convex_error)
}

/// A tracing error as the mesh's.
pub(crate) fn trace_error(e: TraceError) -> MeshError {
    match e {
        TraceError::Num(e) => MeshError::Num(e),
        TraceError::Winding(_) => MeshError::Defect,
        TraceError::Stopped(s) => s.into(),
    }
}

/// A splitting error as the mesh's.
fn convex_error(e: geom_poly::convex::ConvexError) -> MeshError {
    match e {
        geom_poly::convex::ConvexError::Num(e) => MeshError::Num(e),
        geom_poly::convex::ConvexError::Stopped(s) => s.into(),
    }
}
