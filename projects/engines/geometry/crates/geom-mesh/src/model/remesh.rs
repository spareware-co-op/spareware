//! Isotropic remeshing: split, collapse, flip and smooth toward a target edge length, each step
//! kept only inside the two-way envelope (19.6).
//!
//! What it does: [`remesh`] runs PMP's uniform remeshing loop: split edges longer than 4/3 of the
//! target, collapse edges shorter than 4/5 of it, flip edges to bring valences toward 6 (4 on the
//! boundary), and move each vertex toward its triangles' area-weighted centroid in its tangent
//! plane. Each split, collapse, flip and move is a local operation of the module
//! [`crate::model`]: applied, checked against both envelopes, kept or rolled back.
//!
//! Guarantees:
//!
//! - **Certified(ε).** The result is within ε of the input both ways, by the checks of
//!   [`crate::model`]; nothing is projected back onto the input, since the envelope check keeps
//!   every step near it.
//! - **Exact lengths.** "Too long" and "too short" compare squared lengths of grid vectors with
//!   the target's in integers: 9|e|² > 16L² and 25|e|² < 16L².
//! - **On the grid.** A split's new vertex is the edge's middle, each coordinate rounded down; a
//!   move's point is committed to the grid before it is checked.
//! - **Deterministic.** Every pass walks the edges or vertices in number order.
//!
//! Method: Botsch and Kobbelt 2004, "A remeshing approach to multiresolution modeling",
//! <https://doi.org/10.1145/1057432.1057457>, as chapter 19.6 names it
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`).
//! Source: PORT from PMP, MIT,
//!   <https://github.com/pmp-library/pmp-library/blob/215975ca1efafab97477b18187a4a23b87529036/src/pmp/algorithms/remeshing.cpp>
//!   Copyright 2011-2020 the Polygon Mesh Processing Library developers.
//!   Changed for Spare Kit: `uniform_remeshing`, `split_long_edges`, `collapse_short_edges`,
//!   `flip_edges`, `tangential_smoothing` and `weighted_centroid` on a triangle list instead of a
//!   half-edge mesh, with no feature edges or locked vertices; lengths compared exactly in
//!   integers; every edit applied as a local operation and kept only when the two-way envelope
//!   and the checks of the module `model` pass, in place of projecting onto the input with its
//!   kd-tree; smoothing moves one vertex at a time in number order, each move checked, where PMP
//!   moves all at once, and takes the vertex normal as the sum of its triangles' area vectors,
//!   with no least-squares area minimisation first; `remove_caps` is not ported.
//! SPDX-License-Identifier: MIT
//!
//! Cost: each pass is linear in the edges or vertices, times each attempted operation's checks
//! (the module [`crate::model`]); splits and collapses repeat at most 10 passes each, as PMP's.

#![allow(
    clippy::many_single_char_names,
    reason = "the names of the geometry: a triangle's corners and vertices, a normal's parts, a point"
)]

use core::cmp::Ordering;

use geom_num::grid::P3;
use geom_num::math::sqrt;
use geom_run::{Exec, WorkMeter};

use super::{ModelError, Modelled, Surface, length_squared, normal, to_f64, to_grid};

/// The most passes a split or collapse sweep makes, as PMP's.
const PASSES: u32 = 10;

/// What [`remesh`] is asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Remesh {
    /// The envelope's width ε, in grid steps.
    pub epsilon: u64,
    /// The target edge length, in grid steps.
    pub length: u64,
    /// How many rounds of split, collapse, flip and smooth.
    pub iterations: u32,
}

/// The mesh of `triangles` on `points`, remeshed toward edges of `settings.length` inside the
/// two-way envelope of width `settings.epsilon`.
///
/// # Errors
///
/// As for [`super::decimate`].
pub fn remesh(
    points: &[P3],
    triangles: &[[u32; 3]],
    settings: Remesh,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Modelled, ModelError> {
    let mut r = Remesher {
        surface: Surface::new(points, triangles, settings.epsilon, exec, meter)?,
        target: u128::from(settings.length).saturating_mul(u128::from(settings.length)),
    };
    for _ in 0..settings.iterations {
        r.split_long_edges(meter)?;
        r.collapse_short_edges(meter)?;
        r.flip_edges(meter)?;
        r.tangential_smoothing(5, meter)?;
    }
    r.surface.finish()
}

struct Remesher {
    surface: Surface,
    /// L², the target length squared.
    target: u128,
}

impl Remesher {
    /// 9|e|² > 16L²: longer than 4/3 of the target. Both sides below 2^128.
    fn too_long(&self, u: u32, v: u32) -> bool {
        match (self.surface.point(u), self.surface.point(v)) {
            (Some(a), Some(b)) => {
                length_squared(a, b).saturating_mul(9) > self.target.saturating_mul(16)
            }
            _ => false,
        }
    }

    /// 25|e|² < 16L²: shorter than 4/5 of the target.
    fn too_short(&self, u: u32, v: u32) -> bool {
        match (self.surface.point(u), self.surface.point(v)) {
            (Some(a), Some(b)) => {
                length_squared(a, b).saturating_mul(25) < self.target.saturating_mul(16)
            }
            _ => false,
        }
    }

    fn split_long_edges(&mut self, meter: &mut WorkMeter<'_>) -> Result<(), ModelError> {
        for _ in 0..PASSES {
            let mut done = true;
            for (u, v) in self.surface.edges() {
                meter.charge(1)?;
                if !self.too_long(u, v) {
                    continue;
                }
                let (Some(a), Some(b)) = (self.surface.point(u), self.surface.point(v)) else {
                    continue;
                };
                let half = |x: i64, y: i64| x.saturating_add(y).div_euclid(2);
                let Ok(m) = P3::new(
                    half(a.x.get(), b.x.get()),
                    half(a.y.get(), b.y.get()),
                    half(a.z.get(), b.z.get()),
                ) else {
                    continue;
                };
                let Some(patch) = self.surface.split(u, v, m) else {
                    continue;
                };
                if self.surface.attempt(&patch, meter)? {
                    done = false;
                }
            }
            if done {
                break;
            }
        }
        Ok(())
    }

    fn collapse_short_edges(&mut self, meter: &mut WorkMeter<'_>) -> Result<(), ModelError> {
        for _ in 0..PASSES {
            let mut done = true;
            for (v0, v1) in self.surface.edges() {
                meter.charge(1)?;
                if self.surface.edge_faces(v0, v1).is_empty() || !self.too_short(v0, v1) {
                    continue;
                }
                let (b0, b1) = (self.surface.on_boundary(v0), self.surface.on_boundary(v1));
                // Which end may go: (v0 into v1, v1 into v0).
                let (mut go0, mut go1) = (true, true);
                if b0 && b1 {
                    if self.surface.edge_faces(v0, v1).len() != 1 {
                        continue;
                    }
                } else if b0 {
                    go0 = false;
                } else if b1 {
                    go1 = false;
                }
                // Both possible: keep the end of higher valence.
                let (keep, gone) = match (go0, go1) {
                    (true, true) => {
                        if self.surface.neighbours(v0).len() < self.surface.neighbours(v1).len() {
                            (v1, v0)
                        } else {
                            (v0, v1)
                        }
                    }
                    (true, false) => (v1, v0),
                    (false, _) => (v0, v1),
                };
                // Don't create too long edges.
                if self
                    .surface
                    .neighbours(gone)
                    .into_iter()
                    .any(|w| w != keep && self.too_long(keep, w))
                {
                    continue;
                }
                let Some(p) = self.surface.point(keep) else {
                    continue;
                };
                let Some(patch) = self.surface.collapse(keep, gone, p) else {
                    continue;
                };
                if self.surface.attempt(&patch, meter)? {
                    done = false;
                }
            }
            if done {
                break;
            }
        }
        Ok(())
    }

    /// The valence a vertex is best at: 6 inside, 4 on the boundary.
    fn deviation(&self, v: u32, change: i64) -> i64 {
        let best = if self.surface.on_boundary(v) { 4 } else { 6 };
        let valence = i64::try_from(self.surface.neighbours(v).len()).unwrap_or(i64::MAX);
        let d = valence.saturating_add(change).saturating_sub(best);
        d.saturating_mul(d)
    }

    fn flip_edges(&mut self, meter: &mut WorkMeter<'_>) -> Result<(), ModelError> {
        for _ in 0..PASSES {
            let mut done = true;
            for (u, v) in self.surface.edges() {
                meter.charge(1)?;
                let shared = self.surface.edge_faces(u, v);
                let &[f, g] = shared.as_slice() else {
                    continue;
                };
                let third = |t: Option<[u32; 3]>| t?.into_iter().find(|x| *x != u && *x != v);
                let (Some(a), Some(b)) = (third(self.surface.face(f)), third(self.surface.face(g)))
                else {
                    continue;
                };
                let before = self
                    .deviation(u, 0)
                    .saturating_add(self.deviation(v, 0))
                    .saturating_add(self.deviation(a, 0))
                    .saturating_add(self.deviation(b, 0));
                let after = self
                    .deviation(u, -1)
                    .saturating_add(self.deviation(v, -1))
                    .saturating_add(self.deviation(a, 1))
                    .saturating_add(self.deviation(b, 1));
                if before <= after {
                    continue;
                }
                let Some(patch) = self.surface.flip(u, v) else {
                    continue;
                };
                if self.surface.attempt(&patch, meter)? {
                    done = false;
                }
            }
            if done {
                break;
            }
        }
        Ok(())
    }

    fn tangential_smoothing(
        &mut self,
        iterations: u32,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(), ModelError> {
        for _ in 0..iterations {
            for v in 0..u32::try_from(self.surface.vertices()).map_err(|_| ModelError::Full)? {
                meter.charge(1)?;
                if self.surface.faces_at(v).is_empty() || self.surface.on_boundary(v) {
                    continue;
                }
                let Some(p) = self.smoothed(v)? else {
                    continue;
                };
                let Some(patch) = self.surface.shift(v, p) else {
                    continue;
                };
                self.surface.attempt(&patch, meter)?;
            }
        }
        Ok(())
    }

    /// Where smoothing moves `v`: toward its triangles' area-weighted centroid, in the plane
    /// square to its normal, committed to the grid; `None` where that is where it is.
    fn smoothed(&self, v: u32) -> Result<Option<P3>, ModelError> {
        let Some(x) = self.surface.point(v) else {
            return Ok(None);
        };
        let x = to_f64(x);
        let (mut centroid, mut weights, mut n) = ([0.0_f64; 3], 0.0_f64, [0.0_f64; 3]);
        for f in self.surface.faces_at(v) {
            let Some(corners) = self.surface.corners(f) else {
                continue;
            };
            let area_vector = normal(corners).map(i128_to_f64);
            // Twice the area; a triangle with none weighs 1, as in PMP.
            let mut w = sqrt(dot(area_vector, area_vector))?.get();
            if w.partial_cmp(&0.0) != Some(Ordering::Greater) {
                w = 1.0;
            }
            let [a, b, c] = corners.map(to_f64);
            for k in 0..3 {
                if let (Some(s), Some(&pa), Some(&pb), Some(&pc), Some(m), Some(&nv)) = (
                    centroid.get_mut(k),
                    a.get(k),
                    b.get(k),
                    c.get(k),
                    n.get_mut(k),
                    area_vector.get(k),
                ) {
                    *s += w * (pa + pb + pc) / 3.0;
                    *m += nv;
                }
            }
            weights += w;
        }
        if weights.partial_cmp(&0.0) != Some(Ordering::Greater) {
            return Ok(None);
        }
        let length = sqrt(dot(n, n))?.get();
        if length.partial_cmp(&0.0) != Some(Ordering::Greater) {
            return Ok(None);
        }
        let n = n.map(|c| c / length);
        let mut u = [
            centroid[0] / weights - x[0],
            centroid[1] / weights - x[1],
            centroid[2] / weights - x[2],
        ];
        let along = dot(u, n);
        for (c, m) in u.iter_mut().zip(n) {
            *c -= m * along;
        }
        let Some(p) = to_grid([x[0] + u[0], x[1] + u[1], x[2] + u[2]]) else {
            return Ok(None);
        };
        Ok((self.surface.point(v) != Some(p)).then_some(p))
    }
}

/// An area vector's part as a double, rounded once: it only weighs and points the smoothing.
#[allow(
    clippy::cast_precision_loss,
    reason = "a smoothing weight: rounding it changes only where a move is proposed"
)]
fn i128_to_f64(v: i128) -> f64 {
    v as f64
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
