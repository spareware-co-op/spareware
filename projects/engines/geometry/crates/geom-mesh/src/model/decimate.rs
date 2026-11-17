//! Decimation to a bound: edge collapses ranked by quadric error, each kept only inside the
//! two-way envelope (19.6).
//!
//! What it does: [`decimate`] gives each vertex the sum of the squared-distance quadrics of its
//! triangles' planes (and of planes across open boundaries, weighted), and collapses the edge whose
//! ends meet at the point of least summed squared distance first, until the mesh has at most the
//! target number of triangles or no edge can collapse. Each collapse is a local operation of the
//! module [`crate::model`]: applied, checked against both envelopes, kept or rolled back.
//!
//! Guarantees:
//!
//! - **Certified(ε).** The result is within ε of the input both ways, by the checks of
//!   [`crate::model`]; the quadrics only choose the order and the place.
//! - **On the grid.** A collapse's point is the quadric's best point committed to the grid, or an
//!   end or the middle of the edge, also on the grid; the candidates are tried cheapest first.
//! - **Deterministic.** The queue, an ordered set, pops the cheapest candidate, ties broken by
//!   the edge's ends and their versions, so nothing depends on insertion order.
//!
//! Method: Garland and Heckbert 1997, "Surface simplification using quadric error metrics",
//! <https://doi.org/10.1145/258734.258849>, as chapter 19.6 names it
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`).
//! Source: PORT from solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/c248d5c69205f16cd0e492337c74c8ec7ad053bb/crates/solidcore-tess/src/simplify.rs>
//!   Copyright (c) 2026 The solidcore contributors
//!   Changed for Spare Kit: the quadrics, the candidate queue with its stale stamps and its order,
//!   the boundary planes and the choice of point are solidcore's `Quadric` and `Collapser`; a unit
//!   normal's quadric is formed as n nᵀ / (n · n), so no square root is taken; points are grid
//!   points, the best point committed to the grid and tried with the ends and the middle in order
//!   of cost where solidcore takes only the cheapest; its fold test (a normal turning by more than
//!   about 80°) and its shared-neighbour test are replaced by the exact checks and the link
//!   condition of the module `model`, and every collapse must pass the two-way envelope; its
//!   corner welding and its levels with their measured error are dropped, since the input is
//!   already one vertex per point and the result is certified rather than measured.
//! SPDX-License-Identifier: Apache-2.0
//!
//! Cost: each popped candidate costs a collapse's checks (the module [`crate::model`]); the queue
//! holds at most one candidate per edge and change, O(log n) each.

#![allow(
    clippy::many_single_char_names,
    reason = "the names of the geometry: a triangle's corners and vertices, a normal's parts, a point"
)]

use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::cmp::{Ordering, Reverse};

use geom_num::grid::P3;
use geom_run::{Exec, WorkMeter};

use super::{ModelError, Modelled, Surface, normal, to_f64, to_grid};

/// How much more an open boundary's planes count than a face's (solidcore's weight).
const BOUNDARY_WEIGHT: f64 = 10.0;

/// What [`decimate`] is asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decimate {
    /// The envelope's width ε, in grid steps.
    pub epsilon: u64,
    /// Stop once the mesh has at most this many triangles.
    pub target: usize,
}

/// The mesh of `triangles` on `points`, with edges collapsed by quadric error while it has more
/// than `settings.target` triangles and an edge can collapse inside the two-way envelope of width
/// `settings.epsilon`.
///
/// # Errors
///
/// [`ModelError::Index`], [`ModelError::Flat`] and [`ModelError::NotSurface`] for an input that is
/// not an oriented surface; [`ModelError::Full`] for one too large to number; and
/// [`ModelError::Envelope`] when the envelope refuses or the budget runs out.
pub fn decimate(
    points: &[P3],
    triangles: &[[u32; 3]],
    settings: Decimate,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Modelled, ModelError> {
    let surface = Surface::new(points, triangles, settings.epsilon, exec, meter)?;
    let mut c = Collapser::new(surface, meter)?;
    c.collapse_to(settings.target, meter)?;
    c.surface.finish()
}

/// A normal's parts as doubles, each rounded once.
#[allow(
    clippy::cast_precision_loss,
    reason = "a ranking weight: rounding the normal changes only which collapse is tried first"
)]
fn normal_f64(n: [i128; 3]) -> [f64; 3] {
    n.map(|v| v as f64)
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// A symmetric 4 × 4 quadric: a a, a b, a c, a d, b b, b c, b d, c c, c d, d d.
#[derive(Clone, Copy, Debug, Default)]
struct Quadric([f64; 10]);

impl Quadric {
    /// The squared distance to the plane n · p + d = 0, times `w`: solidcore's quadric of the unit
    /// normal n / |n|, formed by dividing by n · n. A zero normal gives the zero quadric.
    fn plane(n: [f64; 3], d: f64, w: f64) -> Quadric {
        let length = dot(n, n);
        // n · n is never negative, so a normal double is one above zero and finite.
        if !length.is_normal() {
            return Quadric::default();
        }
        let s = w / length;
        let [a, b, c] = n;
        Quadric([
            a * a * s,
            a * b * s,
            a * c * s,
            a * d * s,
            b * b * s,
            b * c * s,
            b * d * s,
            c * c * s,
            c * d * s,
            d * d * s,
        ])
    }

    fn add(&mut self, o: &Quadric) {
        for (x, y) in self.0.iter_mut().zip(o.0) {
            *x += y;
        }
    }

    fn sum(&self, o: &Quadric) -> Quadric {
        let mut q = *self;
        q.add(o);
        q
    }

    /// The summed squared distance at `p`.
    fn at(&self, p: [f64; 3]) -> f64 {
        let [aa, ab, ac, ad, bb, bc, bd, cc, cd, dd] = self.0;
        let [x, y, z] = p;
        let v = aa * x * x
            + 2.0 * ab * x * y
            + 2.0 * ac * x * z
            + 2.0 * ad * x
            + bb * y * y
            + 2.0 * bc * y * z
            + 2.0 * bd * y
            + cc * z * z
            + 2.0 * cd * z
            + dd;
        if v.partial_cmp(&0.0) == Some(Ordering::Greater) {
            v
        } else {
            0.0
        }
    }

    /// The point of least summed squared distance, when it is well defined.
    fn best(&self) -> Option<[f64; 3]> {
        let [aa, ab, ac, ad, bb, bc, bd, cc, cd, _] = self.0;
        let det = aa * (bb * cc - bc * bc) - ab * (ab * cc - bc * ac) + ac * (ab * bc - bb * ac);
        let scale = aa.abs() + bb.abs() + cc.abs();
        let scale = scale * scale * scale;
        if det.abs().partial_cmp(&(1e-12 * scale)) != Some(Ordering::Greater) {
            return None;
        }
        let (r0, r1, r2) = (-ad, -bd, -cd);
        let x =
            (r0 * (bb * cc - bc * bc) - ab * (r1 * cc - bc * r2) + ac * (r1 * bc - bb * r2)) / det;
        let y =
            (aa * (r1 * cc - bc * r2) - r0 * (ab * cc - bc * ac) + ac * (ab * r2 - r1 * ac)) / det;
        let z =
            (aa * (bb * r2 - r1 * bc) - ab * (ab * r2 - r1 * ac) + r0 * (ab * bc - bb * ac)) / det;
        [x, y, z].iter().all(|v| v.is_finite()).then_some([x, y, z])
    }
}

/// A candidate collapse: its cost, its ends, and their versions when it was costed (a later
/// change to either end makes it stale). Ordered by those fields in turn, the latest versions
/// first, so the queue's least is the cheapest and no two candidates are equal unless they are
/// the same. The cost is held as its bit pattern: a cost is never negative or NaN
/// ([`Quadric::at`]), and the bit patterns of such doubles rise as the numbers do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Candidate {
    cost: u64,
    u: u32,
    v: u32,
    stamps: Reverse<(u32, u32)>,
}

struct Collapser {
    surface: Surface,
    quadric: Vec<Quadric>,
    stamp: Vec<u32>,
    queue: BTreeSet<Candidate>,
}

impl Collapser {
    fn new(surface: Surface, meter: &mut WorkMeter<'_>) -> Result<Collapser, ModelError> {
        let n = surface.vertices();
        let mut quadric = alloc::vec![Quadric::default(); n];
        let mut sides: Vec<(u32, u32, u32)> = Vec::new();
        for f in 0..u32::try_from(surface.faces.len()).map_err(|_| ModelError::Full)? {
            meter.charge(1)?;
            let (Some(t), Some(corners)) = (surface.face(f), surface.corners(f)) else {
                continue;
            };
            let nrm = normal_f64(normal(corners));
            let q = Quadric::plane(nrm, -dot(nrm, to_f64(corners[0])), 1.0);
            for &i in &t {
                if let Some(x) = quadric.get_mut(i as usize) {
                    x.add(&q);
                }
            }
            let [a, b, c] = t;
            for (i, j) in [(a, b), (b, c), (c, a)] {
                sides.push((i.min(j), i.max(j), f));
            }
        }
        // Open boundaries (sides of one triangle only): a plane across each, weighted, so the
        // boundary keeps its place.
        sides.sort_unstable();
        for (k, &(a, b, f)) in sides.iter().enumerate() {
            let same =
                |other: Option<&(u32, u32, u32)>| other.is_some_and(|o| (o.0, o.1) == (a, b));
            let alone = !same(k.checked_sub(1).and_then(|k| sides.get(k)))
                && !same(sides.get(k.saturating_add(1)));
            if alone
                && let (Some(pa), Some(pb), Some(corners)) =
                    (surface.point(a), surface.point(b), surface.corners(f))
            {
                let face = normal_f64(normal(corners));
                let (pa, pb) = (to_f64(pa), to_f64(pb));
                let across = cross(sub(pb, pa), face);
                let q = Quadric::plane(across, -dot(across, pa), BOUNDARY_WEIGHT);
                for i in [a, b] {
                    if let Some(x) = quadric.get_mut(i as usize) {
                        x.add(&q);
                    }
                }
            }
        }
        let mut c = Collapser {
            surface,
            quadric,
            stamp: alloc::vec![0; n],
            queue: BTreeSet::new(),
        };
        let mut last = None;
        for &(a, b, _) in &sides {
            if last != Some((a, b)) {
                c.offer(a, b);
                last = Some((a, b));
            }
        }
        Ok(c)
    }

    /// The summed quadric of `u` and `v`.
    fn joined(&self, u: u32, v: u32) -> Option<Quadric> {
        let (qu, qv) = (self.quadric.get(u as usize)?, self.quadric.get(v as usize)?);
        Some(qu.sum(qv))
    }

    /// Where `u` and `v` could merge, on the grid, cheapest first, with each cost: the best point
    /// when it lies near the edge, the ends and the middle.
    fn targets(&self, u: u32, v: u32) -> Option<Vec<(P3, f64)>> {
        let q = self.joined(u, v)?;
        let (pu, pv) = (self.surface.point(u)?, self.surface.point(v)?);
        let (fu, fv) = (to_f64(pu), to_f64(pv));
        let mid = [
            f64::midpoint(fu[0], fv[0]),
            f64::midpoint(fu[1], fv[1]),
            f64::midpoint(fu[2], fv[2]),
        ];
        let reach = dot(sub(fu, fv), sub(fu, fv)) * 4.0;
        let best = q
            .best()
            .filter(|p| dot(sub(*p, mid), sub(*p, mid)) <= reach)
            .and_then(to_grid);
        let mut out: Vec<(P3, f64)> = Vec::with_capacity(4);
        for p in best.into_iter().chain([pu, pv]).chain(to_grid(mid)) {
            if out.iter().all(|(x, _)| *x != p) {
                out.push((p, q.at(to_f64(p))));
            }
        }
        // Stable: equal costs keep the order above.
        out.sort_by(|a, b| a.1.total_cmp(&b.1));
        Some(out)
    }

    fn offer(&mut self, u: u32, v: u32) {
        let Some(cost) = self
            .targets(u, v)
            .and_then(|t| t.first().map(|(_, cost)| *cost))
        else {
            return;
        };
        let stamps = Reverse((
            self.stamp.get(u as usize).copied().unwrap_or(0),
            self.stamp.get(v as usize).copied().unwrap_or(0),
        ));
        self.queue.insert(Candidate {
            cost: cost.to_bits(),
            u,
            v,
            stamps,
        });
    }

    fn collapse_to(&mut self, target: usize, meter: &mut WorkMeter<'_>) -> Result<(), ModelError> {
        while self.surface.live() > target {
            let Some(c) = self.queue.pop_first() else {
                return Ok(());
            };
            meter.charge(1)?;
            let stamp = |i: u32| self.stamp.get(i as usize).copied().unwrap_or(u32::MAX);
            if Reverse((stamp(c.u), stamp(c.v))) != c.stamps
                || self.surface.faces_at(c.v).is_empty()
            {
                continue;
            }
            self.collapse(c.u, c.v, meter)?;
        }
        Ok(())
    }

    /// Merge `v` into `u` at the cheapest of their points the checks keep.
    fn collapse(&mut self, u: u32, v: u32, meter: &mut WorkMeter<'_>) -> Result<(), ModelError> {
        let Some(targets) = self.targets(u, v) else {
            return Ok(());
        };
        for (p, _) in targets {
            let Some(patch) = self.surface.collapse(u, v, p) else {
                // The topology refuses whatever the point.
                return Ok(());
            };
            if self.surface.attempt(&patch, meter)? {
                let qv = self.quadric.get(v as usize).copied().unwrap_or_default();
                if let Some(q) = self.quadric.get_mut(u as usize) {
                    q.add(&qv);
                }
                for i in [u, v] {
                    if let Some(stamp) = self.stamp.get_mut(i as usize) {
                        *stamp = stamp.wrapping_add(1);
                    }
                }
                for w in self.surface.neighbours(u) {
                    self.offer(u.min(w), u.max(w));
                }
                return Ok(());
            }
        }
        Ok(())
    }
}
