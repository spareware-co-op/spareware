//! Mesh modelling on triangle meshes: decimation and remeshing inside a two-way envelope (19.6).
//!
//! What it does: [`decimate`] collapses edges by quadric error until a triangle count is reached,
//! and [`remesh`] splits, collapses, flips and smooths toward a target edge length. Both edit a
//! triangle mesh on grid points one local operation at a time. Each operation is applied, checked
//! and rolled back if a check fails, the pattern of wildmeshing-toolkit that 19.6 names. A
//! [`Modelled`] mesh is what remains.
//!
//! Guarantees:
//!
//! - **Certified(ε), both ways.** Every point of the result lies within ε of the input, and every
//!   point of the input within ε of the result. An operation is kept only when
//!   [`crate::envelope::covers`] proves each new triangle inside the input's envelope
//!   ([`Envelope`]), and [`covers_within`] proves the part of each input triangle inside each
//!   removed triangle's body inside the envelope of the result's triangles. Initially each
//!   triangle covers itself; an operation takes coverage away only inside the bodies it removed,
//!   and those parts are rechecked; so both inclusions hold after every operation, by induction.
//!   Inside an envelope proves within ε (the module [`crate::envelope`]), so the two-way
//!   (Hausdorff) distance is at most ε: [`Modelled::exactness`].
//! - **Exact decisions.** Every check is exact: integer orientation and normal signs, and the
//!   envelope's plane signs. Floating point only ranks candidates and proposes positions, which
//!   are committed to the grid before anything is checked, so no rounding decides anything.
//! - **The surface stays a surface.** The input must be an oriented surface, closed or with
//!   boundary: every edge in one or two triangles, run opposite ways, and the triangles at each
//!   vertex one fan. Every operation keeps that (the link condition and its boundary rules), never
//!   makes a triangle without area or a second triangle on the same corners, and never turns a
//!   triangle's normal by a right angle or more from the triangle it replaces. A self-crossing
//!   result is not ruled out: two sheets of the input closer than 2ε may cross.
//! - **Deterministic.** Every list is ordered by vertex and triangle numbers; candidates tie by
//!   their ends; no hash table is iterated.
//! - **Bounded.** Every operation is charged to the meter, so a budget ends a run with an error,
//!   never a hang.
//!
//! Method: chapter 19.6's table, "Decimation to a bound" and "Isotropic remeshing"
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`): local operations with
//! invariant checks and rollback, as wildmeshing-toolkit builds them, under the exact envelope
//! test of 19.8 (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`). A collapse
//! keeps the link condition of Dey, Edelsbrunner, Guha and Nekhayev (1999), with the boundary
//! rule of the PMP library's remeshing (the module [`remesh`]).
//!
//! Cost: each operation costs its envelope tests: one [`crate::envelope::covers`] per new triangle
//! against the input bodies near it, and one [`covers_within`] per input triangle and removed body
//! that may meet, against the result triangles whose bodies' boxes meet that body's, found by a
//! scan of every result triangle's box, so linear in the mesh for each. Each triangle's body is
//! made once, when the triangle is. Memory is linear in the mesh.

#![allow(
    clippy::many_single_char_names,
    reason = "the names of the geometry: a triangle's corners and vertices, a normal's parts, a point"
)]

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;

use geom_num::grid::{GridStep, P3, Steps};
use geom_num::math::round_ties_even;
use geom_num::wide::I256;
use geom_num::{Exactness, NumError, Sign};
use geom_run::{Exec, Stopped, WorkMeter};

use crate::envelope::{Envelope, EnvelopeError, TriangleEnvelope, covers_within, half_side};

pub mod decimate;
pub mod remesh;

pub use decimate::{Decimate, decimate};
pub use remesh::{Remesh, remesh};

/// Why a mesh could not be modelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelError {
    /// Triangle `face` names a vertex that is not in the list.
    Index {
        /// The triangle's position in the input.
        face: u32,
    },
    /// Triangle `face` repeats a corner or has no area.
    Flat {
        /// The triangle's position in the input.
        face: u32,
    },
    /// Triangle `face` joins an edge that already has two triangles or runs it the way another
    /// does, or a vertex of it has triangles that are not one fan.
    NotSurface {
        /// The triangle's position in the input.
        face: u32,
    },
    /// More vertices or triangles than a `u32` numbers.
    Full,
    /// The envelope refused: a plane over budget, the tree, or the budget.
    Envelope(EnvelopeError),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelError::Index { face } => write!(f, "triangle {face} names a missing vertex"),
            ModelError::Flat { face } => write!(f, "triangle {face} has no area"),
            ModelError::NotSurface { face } => {
                write!(f, "triangle {face} does not join the others as a surface")
            }
            ModelError::Full => f.write_str("more vertices or triangles than a u32 numbers"),
            ModelError::Envelope(e) => write!(f, "{e}"),
        }
    }
}

impl core::error::Error for ModelError {}

impl From<EnvelopeError> for ModelError {
    fn from(e: EnvelopeError) -> Self {
        ModelError::Envelope(e)
    }
}

impl From<NumError> for ModelError {
    fn from(e: NumError) -> Self {
        ModelError::Envelope(EnvelopeError::Num(e))
    }
}

impl From<Stopped> for ModelError {
    fn from(e: Stopped) -> Self {
        ModelError::Envelope(EnvelopeError::Stopped(e))
    }
}

/// A mesh decimated or remeshed within ε of its input, both ways.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Modelled {
    points: Vec<P3>,
    triangles: Vec<[u32; 3]>,
    epsilon: u64,
    kept: u64,
    refused: u64,
}

impl Modelled {
    /// The vertices, each used by a triangle, in the order of their numbers in the input (new
    /// vertices after them, in the order they were made).
    #[must_use]
    pub fn points(&self) -> &[P3] {
        &self.points
    }

    /// The triangles, counter-clockwise seen from outside as the input's were.
    #[must_use]
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }

    /// The envelope's width ε, in grid steps: the result and the input are within it, both ways.
    #[must_use]
    pub const fn epsilon(&self) -> u64 {
        self.epsilon
    }

    /// How many operations were kept, and how many the checks rolled back.
    #[must_use]
    pub const fn operations(&self) -> (u64, u64) {
        (self.kept, self.refused)
    }

    /// The result's class against the input's, on a grid of `step`: Certified(ε).
    #[must_use]
    pub const fn exactness(&self, step: GridStep) -> Exactness {
        Exactness::Certified {
            bound: Steps(self.epsilon),
            step,
        }
    }
}

/// A local operation: triangles removed, triangles added, vertices moved and vertices made.
#[derive(Clone, Debug, Default)]
pub(crate) struct Patch {
    /// The triangles removed, by number.
    remove: Vec<u32>,
    /// Each triangle added, with the two removed triangles (positions in `remove`) whose normals
    /// its own must agree with; one triangle given twice when it replaces one.
    add: Vec<([u32; 3], [usize; 2])>,
    /// Vertices moved, and where to.
    moves: Vec<(u32, P3)>,
    /// New vertices, numbered on from the vertices there are.
    fresh: Vec<P3>,
}

/// What a patch removed, as it was: the triangles' normals and bodies, and their box.
#[derive(Clone, Copy)]
struct Removed<'a> {
    normals: &'a [[i128; 3]],
    bodies: &'a [TriangleEnvelope],
    low: [i64; 3],
    high: [i64; 3],
}

/// What [`Surface::apply`] changed, to put it back.
struct Undo {
    removed: Vec<u32>,
    added: usize,
    moved: Vec<(u32, P3)>,
    fresh: usize,
}

/// Coordinate `k` of a point.
fn coordinate(p: P3, k: usize) -> i128 {
    i128::from(match k {
        0 => p.x.get(),
        1 => p.y.get(),
        _ => p.z.get(),
    })
}

/// `b − a`: each part below 2^41.
fn run(a: P3, b: P3) -> [i128; 3] {
    let part = |k: usize| coordinate(b, k).wrapping_sub(coordinate(a, k));
    [part(0), part(1), part(2)]
}

/// (b − a) × (c − a): each part below 2^83, so the wrapping steps are exact.
pub(crate) fn normal(t: [P3; 3]) -> [i128; 3] {
    let [a, b, c] = t;
    let ([x, y, z], [u, v, w]) = (run(a, b), run(a, c));
    let term =
        |p: i128, q: i128, r: i128, s: i128| p.wrapping_mul(q).wrapping_sub(r.wrapping_mul(s));
    [term(y, w, z, v), term(z, u, x, w), term(x, v, y, u)]
}

/// The sign of m · n for parts below 2^83: each product below 2^166, in [`I256`].
fn dot_sign(m: [i128; 3], n: [i128; 3]) -> Option<Sign> {
    let mut sum = I256::from_i128(0);
    for (a, b) in m.into_iter().zip(n) {
        let product = I256::from_i128(a).checked_mul(&I256::from_i128(b))?;
        sum = sum.checked_add(&product)?;
    }
    Some(sum.signum())
}

/// |b − a|², below 2^84.
pub(crate) fn length_squared(a: P3, b: P3) -> u128 {
    let [x, y, z] = run(a, b).map(i128::unsigned_abs);
    x.wrapping_mul(x)
        .wrapping_add(y.wrapping_mul(y))
        .wrapping_add(z.wrapping_mul(z))
}

/// The corners' box, `low` and `high` widened to hold it.
fn widen(low: &mut [i64; 3], high: &mut [i64; 3], p: P3) {
    for (k, v) in [p.x.get(), p.y.get(), p.z.get()].into_iter().enumerate() {
        if let (Some(l), Some(h)) = (low.get_mut(k), high.get_mut(k)) {
            *l = (*l).min(v);
            *h = (*h).max(v);
        }
    }
}

/// A point as doubles: exact, since every coordinate is below 2^40.
#[allow(
    clippy::cast_precision_loss,
    reason = "grid coordinates are below 2^40 in magnitude, so each is a double exactly"
)]
pub(crate) fn to_f64(p: P3) -> [f64; 3] {
    [p.x.get() as f64, p.y.get() as f64, p.z.get() as f64]
}

/// A double committed to the grid: the nearest whole number, ties to even, if inside the frame.
pub(crate) fn to_grid(p: [f64; 3]) -> Option<P3> {
    let coordinate = |v: f64| -> Option<i64> {
        let r = round_ties_even(v).ok()?.get();
        // The frame is below 2^40 in magnitude; a whole double inside it converts exactly.
        if r.abs().partial_cmp(&1_099_511_627_776.0) != Some(Ordering::Less) {
            return None;
        }
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a whole number below 2^40 in magnitude, checked above"
        )]
        Some(r as i64)
    };
    let [x, y, z] = p;
    P3::new(coordinate(x)?, coordinate(y)?, coordinate(z)?).ok()
}

/// A triangle mesh being edited, with the input's envelope it must stay inside.
pub(crate) struct Surface {
    points: Vec<P3>,
    faces: Vec<[u32; 3]>,
    alive: Vec<bool>,
    /// The triangles at each vertex; dead ones are dropped when an operation is kept.
    around: Vec<Vec<u32>>,
    /// Each triangle's body, of half side `h`, made when the triangle was.
    shell: Vec<TriangleEnvelope>,
    live: usize,
    /// The input's triangles and their envelope.
    source: Vec<[P3; 3]>,
    input: Envelope,
    h: u64,
    epsilon: u64,
    kept: u64,
    refused: u64,
}

impl Surface {
    /// The mesh of `triangles` on `points`, checked to be an oriented surface, with the envelope
    /// of width `epsilon` round it.
    pub(crate) fn new(
        points: &[P3],
        triangles: &[[u32; 3]],
        epsilon: u64,
        exec: &Exec<'_>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Self, ModelError> {
        u32::try_from(points.len()).map_err(|_| ModelError::Full)?;
        u32::try_from(triangles.len()).map_err(|_| ModelError::Full)?;
        let mut source = Vec::with_capacity(triangles.len());
        let mut around: Vec<Vec<u32>> = alloc::vec![Vec::new(); points.len()];
        for (t, face) in triangles.iter().zip(0_u32..) {
            meter.charge(1)?;
            let corner = |i: u32| points.get(i as usize).copied();
            let [Some(a), Some(b), Some(c)] = t.map(corner) else {
                return Err(ModelError::Index { face });
            };
            if normal([a, b, c]) == [0, 0, 0] {
                return Err(ModelError::Flat { face });
            }
            source.push([a, b, c]);
            for &i in t {
                if let Some(list) = around.get_mut(i as usize) {
                    list.push(face);
                }
            }
        }
        check_surface(triangles, &around, meter)?;
        let input = Envelope::new(&source, epsilon, exec, meter)?;
        let h = half_side(epsilon);
        let mut shell = Vec::with_capacity(source.len());
        for t in &source {
            meter.charge(1)?;
            shell.push(TriangleEnvelope::new(*t, h)?);
        }
        Ok(Self {
            points: points.to_vec(),
            faces: triangles.to_vec(),
            alive: alloc::vec![true; triangles.len()],
            around,
            shell,
            live: triangles.len(),
            source,
            input,
            h,
            epsilon,
            kept: 0,
            refused: 0,
        })
    }

    /// How many triangles there are.
    pub(crate) const fn live(&self) -> usize {
        self.live
    }

    /// How many vertices there have been, dead ones included.
    pub(crate) fn vertices(&self) -> usize {
        self.points.len()
    }

    /// Vertex `v`'s position.
    pub(crate) fn point(&self, v: u32) -> Option<P3> {
        self.points.get(v as usize).copied()
    }

    /// Triangle `f`'s corners, if it is alive.
    pub(crate) fn face(&self, f: u32) -> Option<[u32; 3]> {
        match self.alive.get(f as usize) {
            Some(true) => self.faces.get(f as usize).copied(),
            _ => None,
        }
    }

    /// Triangle `f`'s corner positions.
    pub(crate) fn corners(&self, f: u32) -> Option<[P3; 3]> {
        let [a, b, c] = self.faces.get(f as usize)?.map(|v| self.point(v));
        Some([a?, b?, c?])
    }

    /// The live triangles at vertex `v`, rising.
    pub(crate) fn faces_at(&self, v: u32) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .around
            .get(v as usize)
            .into_iter()
            .flatten()
            .copied()
            .filter(|f| self.face(*f).is_some())
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// The live triangles with edge `u`–`v`, rising.
    pub(crate) fn edge_faces(&self, u: u32, v: u32) -> Vec<u32> {
        let mut out = self.faces_at(u);
        out.retain(|f| self.face(*f).is_some_and(|t| t.contains(&v)));
        out
    }

    /// The vertices that share a live triangle with `v`, rising.
    pub(crate) fn neighbours(&self, v: u32) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .faces_at(v)
            .into_iter()
            .filter_map(|f| self.face(f))
            .flatten()
            .filter(|w| *w != v)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Whether `v` is on the boundary: one fan with a gap has a neighbour more than triangles.
    pub(crate) fn on_boundary(&self, v: u32) -> bool {
        self.neighbours(v).len() > self.faces_at(v).len()
    }

    /// Every edge of a live triangle, as rising pairs, in order.
    pub(crate) fn edges(&self) -> Vec<(u32, u32)> {
        let mut out = Vec::with_capacity(self.live.saturating_mul(3));
        for f in 0..self.faces.len() {
            let Some([a, b, c]) = u32::try_from(f).ok().and_then(|f| self.face(f)) else {
                continue;
            };
            for (u, v) in [(a, b), (b, c), (c, a)] {
                out.push((u.min(v), u.max(v)));
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Merging `gone` into `keep` at `p`, if the link condition and its boundary rules allow it:
    /// the edge is in one or two triangles, the ends share no neighbour but those triangles'
    /// third corners, and two boundary ends are joined only along a boundary edge.
    pub(crate) fn collapse(&self, keep: u32, gone: u32, p: P3) -> Option<Patch> {
        let shared = self.edge_faces(keep, gone);
        if shared.is_empty() || shared.len() > 2 {
            return None;
        }
        let (nk, ng) = (self.neighbours(keep), self.neighbours(gone));
        let common = nk.iter().filter(|w| ng.binary_search(w).is_ok()).count();
        if common != shared.len() {
            return None;
        }
        if self.on_boundary(keep) && self.on_boundary(gone) && shared.len() != 1 {
            return None;
        }
        let mut remove = self.faces_at(keep);
        remove.extend(self.faces_at(gone));
        remove.sort_unstable();
        remove.dedup();
        let mut add = Vec::with_capacity(remove.len());
        for (k, &f) in remove.iter().enumerate() {
            let t = self.face(f)?;
            if t.contains(&keep) && t.contains(&gone) {
                continue;
            }
            add.push((t.map(|v| if v == gone { keep } else { v }), [k, k]));
        }
        Some(Patch {
            remove,
            add,
            moves: alloc::vec![(keep, p)],
            fresh: Vec::new(),
        })
    }

    /// Splitting edge `u`–`v` at a new vertex at `m`: each triangle on the edge becomes two.
    pub(crate) fn split(&self, u: u32, v: u32, m: P3) -> Option<Patch> {
        let shared = self.edge_faces(u, v);
        if shared.is_empty() || shared.len() > 2 {
            return None;
        }
        let w = u32::try_from(self.points.len()).ok()?;
        let mut add = Vec::with_capacity(4);
        for (k, &f) in shared.iter().enumerate() {
            let t = self.face(f)?;
            // Turn the triangle so the edge comes first, as it runs: (a, b, x).
            let [a, b, x] = match t {
                [p, q, r] if (p, q) == (u, v) || (p, q) == (v, u) => [p, q, r],
                [r, p, q] if (p, q) == (u, v) || (p, q) == (v, u) => [p, q, r],
                [q, r, p] => [p, q, r],
            };
            add.push(([a, w, x], [k, k]));
            add.push(([w, b, x], [k, k]));
        }
        Some(Patch {
            remove: shared,
            add,
            moves: Vec::new(),
            fresh: alloc::vec![m],
        })
    }

    /// Flipping edge `u`–`v` between its two triangles to join their third corners, when they are
    /// different and not joined already.
    pub(crate) fn flip(&self, u: u32, v: u32) -> Option<Patch> {
        let shared = self.edge_faces(u, v);
        let &[f, g] = shared.as_slice() else {
            return None;
        };
        let third = |t: [u32; 3]| t.into_iter().find(|x| *x != u && *x != v);
        let (s, t) = (self.face(f)?, self.face(g)?);
        let (a, b) = (third(s)?, third(t)?);
        if a == b || !self.edge_faces(a, b).is_empty() {
            return None;
        }
        // Turn `s` so it runs (p, q, a) with p → q its edge; `t` runs (q, p, b).
        let (p, q) = match s {
            [x, y, _] if x != a && y != a => (x, y),
            [x, _, y] if x != a && y != a => (y, x),
            [_, x, y] => (x, y),
        };
        Some(Patch {
            remove: alloc::vec![f, g],
            add: alloc::vec![([p, b, a], [0, 1]), ([b, q, a], [0, 1])],
            moves: Vec::new(),
            fresh: Vec::new(),
        })
    }

    /// Moving vertex `v` to `p`: its triangles are replaced by themselves, moved.
    pub(crate) fn shift(&self, v: u32, p: P3) -> Option<Patch> {
        let remove = self.faces_at(v);
        let mut add = Vec::with_capacity(remove.len());
        for (k, &f) in remove.iter().enumerate() {
            add.push((self.face(f)?, [k, k]));
        }
        Some(Patch {
            remove,
            add,
            moves: alloc::vec![(v, p)],
            fresh: Vec::new(),
        })
    }

    /// Apply `patch`, check it, and keep it or roll it back: whether it was kept.
    pub(crate) fn attempt(
        &mut self,
        patch: &Patch,
        meter: &mut WorkMeter<'_>,
    ) -> Result<bool, ModelError> {
        meter.charge(1)?;
        let mut before = Vec::with_capacity(patch.remove.len());
        let mut gone = Vec::with_capacity(patch.remove.len());
        let (mut low, mut high) = ([i64::MAX; 3], [i64::MIN; 3]);
        for &f in &patch.remove {
            meter.charge(1)?;
            let corners = self.corners(f).ok_or(ModelError::Full)?;
            before.push(normal(corners));
            gone.push(self.shell.get(f as usize).cloned().ok_or(ModelError::Full)?);
            for p in corners {
                widen(&mut low, &mut high, p);
            }
        }
        let (undo, added) = self.apply(patch)?;
        for &f in &added {
            let body = self
                .corners(f)
                .ok_or(ModelError::Full)
                .and_then(|c| Ok(TriangleEnvelope::new(c, self.h)?));
            match body {
                Ok(body) => self.shell.push(body),
                Err(e) => {
                    self.undo(undo);
                    return Err(e);
                }
            }
        }
        let removed = Removed {
            normals: &before,
            bodies: &gone,
            low,
            high,
        };
        let kept = match self.check(&added, &patch.add, &removed, meter) {
            Ok(kept) => kept,
            Err(e) => {
                self.undo(undo);
                return Err(e);
            }
        };
        if kept {
            self.kept = self.kept.saturating_add(1);
            for &f in patch.remove.iter().chain(&added) {
                if let Some(t) = self.faces.get(f as usize).copied() {
                    for v in t {
                        let alive = &self.alive;
                        if let Some(list) = self.around.get_mut(v as usize) {
                            list.retain(|g| alive.get(*g as usize).copied().unwrap_or(false));
                        }
                    }
                }
            }
        } else {
            self.refused = self.refused.saturating_add(1);
            self.undo(undo);
        }
        Ok(kept)
    }

    /// Apply `patch`: the numbers of the triangles it added, and how to undo it.
    fn apply(&mut self, patch: &Patch) -> Result<(Undo, Vec<u32>), ModelError> {
        for &p in &patch.fresh {
            u32::try_from(self.points.len()).map_err(|_| ModelError::Full)?;
            self.points.push(p);
            self.around.push(Vec::new());
        }
        let mut moved = Vec::with_capacity(patch.moves.len());
        for &(v, p) in &patch.moves {
            if let Some(slot) = self.points.get_mut(v as usize) {
                moved.push((v, *slot));
                *slot = p;
            }
        }
        for &f in &patch.remove {
            if let Some(a) = self.alive.get_mut(f as usize) {
                *a = false;
                self.live = self.live.saturating_sub(1);
            }
        }
        let mut added = Vec::with_capacity(patch.add.len());
        for (t, _) in &patch.add {
            let f = u32::try_from(self.faces.len()).map_err(|_| ModelError::Full)?;
            self.faces.push(*t);
            self.alive.push(true);
            for &v in t {
                if let Some(list) = self.around.get_mut(v as usize) {
                    list.push(f);
                }
            }
            self.live = self.live.saturating_add(1);
            added.push(f);
        }
        let undo = Undo {
            removed: patch.remove.clone(),
            added: added.len(),
            moved,
            fresh: patch.fresh.len(),
        };
        Ok((undo, added))
    }

    /// Put back what [`Surface::apply`] changed, in the reverse order.
    fn undo(&mut self, undo: Undo) {
        for _ in 0..undo.added {
            if let Some(t) = self.faces.pop() {
                self.alive.pop();
                // Each corner's list got this triangle last.
                for v in t {
                    if let Some(list) = self.around.get_mut(v as usize) {
                        list.pop();
                    }
                }
                self.live = self.live.saturating_sub(1);
            }
        }
        for f in undo.removed {
            if let Some(a) = self.alive.get_mut(f as usize) {
                *a = true;
                self.live = self.live.saturating_add(1);
            }
        }
        for (v, p) in undo.moved.into_iter().rev() {
            if let Some(slot) = self.points.get_mut(v as usize) {
                *slot = p;
            }
        }
        for _ in 0..undo.fresh {
            self.points.pop();
            self.around.pop();
        }
        self.shell.truncate(self.faces.len());
    }

    /// The checks of an applied patch: each added triangle has area, is on no other triangle's
    /// corners and turns its normal by less than a right angle from those it replaces; each is
    /// inside the input's envelope; and each input triangle that may meet a removed triangle's
    /// body is inside the envelope of the triangles near the added ones. An input triangle that
    /// meets no removed body was covered by bodies that are all still there.
    fn check(
        &self,
        added: &[u32],
        plan: &[([u32; 3], [usize; 2])],
        removed: &Removed<'_>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<bool, ModelError> {
        let Removed {
            normals: before,
            bodies: gone,
            low,
            high,
        } = *removed;
        for (&f, (t, parents)) in added.iter().zip(plan) {
            meter.charge(1)?;
            let corners = self.corners(f).ok_or(ModelError::Full)?;
            let n = normal(corners);
            if n == [0, 0, 0] {
                return Ok(false);
            }
            for k in parents {
                let m = before.get(*k).copied().ok_or(ModelError::Full)?;
                if dot_sign(n, m) != Some(Sign::Positive) {
                    return Ok(false);
                }
            }
            let mut set = *t;
            set.sort_unstable();
            for g in self.faces_at(t[0]) {
                if g != f
                    && self.face(g).is_some_and(|mut u| {
                        u.sort_unstable();
                        u == set
                    })
                {
                    return Ok(false);
                }
            }
        }
        for &f in added {
            let corners = self.corners(f).ok_or(ModelError::Full)?;
            if !self.input.covers(corners, meter)? {
                return Ok(false);
            }
        }
        if low[0] > high[0] {
            return Ok(true);
        }
        for key in self.input.near_keys(low, high, meter)? {
            let t = usize::try_from(key)
                .ok()
                .and_then(|k| self.source.get(k))
                .ok_or(ModelError::Full)?;
            // The part of it inside each removed body must be covered by the result's bodies
            // that reach that body's box.
            for body in gone {
                meter.charge(1)?;
                if !body.may_meet(*t)? {
                    continue;
                }
                let (low, high) = body.bounds();
                let mut refs: Vec<&TriangleEnvelope> = Vec::new();
                for (other, alive) in self.shell.iter().zip(&self.alive) {
                    meter.charge(1)?;
                    if *alive && other.reaches(low, high) {
                        refs.push(other);
                    }
                }
                if !covers_within(&refs, *t, Some(body), meter)? {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// The mesh as it stands: live vertices in number order, live triangles in number order.
    pub(crate) fn finish(&self) -> Result<Modelled, ModelError> {
        let mut index: Vec<Option<u32>> = alloc::vec![None; self.points.len()];
        let mut used: Vec<bool> = alloc::vec![false; self.points.len()];
        for f in 0..self.faces.len() {
            if let Some(t) = u32::try_from(f).ok().and_then(|f| self.face(f)) {
                for v in t {
                    if let Some(u) = used.get_mut(v as usize) {
                        *u = true;
                    }
                }
            }
        }
        let mut points = Vec::new();
        for ((slot, &u), &p) in index.iter_mut().zip(&used).zip(&self.points) {
            if u {
                *slot = Some(u32::try_from(points.len()).map_err(|_| ModelError::Full)?);
                points.push(p);
            }
        }
        let mut triangles = Vec::with_capacity(self.live);
        for f in 0..self.faces.len() {
            if let Some(t) = u32::try_from(f).ok().and_then(|f| self.face(f)) {
                let [Some(a), Some(b), Some(c)] =
                    t.map(|v| index.get(v as usize).copied().flatten())
                else {
                    return Err(ModelError::Full);
                };
                triangles.push([a, b, c]);
            }
        }
        Ok(Modelled {
            points,
            triangles,
            epsilon: self.epsilon,
            kept: self.kept,
            refused: self.refused,
        })
    }
}

/// Check that `triangles` form an oriented surface: no directed edge twice, and the triangles at
/// each vertex one fan. With each directed edge once, an edge has at most two triangles, run
/// opposite ways, and the link of each vertex (the edge opposite it in each of its triangles) is
/// paths and cycles; one fan is one path or one cycle.
fn check_surface(
    triangles: &[[u32; 3]],
    around: &[Vec<u32>],
    meter: &mut WorkMeter<'_>,
) -> Result<(), ModelError> {
    let mut directed: Vec<(u32, u32, u32)> = Vec::with_capacity(triangles.len().saturating_mul(3));
    for (&[a, b, c], face) in triangles.iter().zip(0_u32..) {
        if a == b || b == c || c == a {
            return Err(ModelError::Flat { face });
        }
        directed.extend([(a, b, face), (b, c, face), (c, a, face)]);
    }
    directed.sort_unstable();
    for pair in directed.windows(2) {
        if let [(a, b, _), (c, d, face)] = pair
            && (a, b) == (c, d)
        {
            return Err(ModelError::NotSurface { face: *face });
        }
    }
    for (v, faces) in (0_u32..).zip(around) {
        meter.charge(1)?;
        // The link's edges: x → y for each triangle (v, x, y).
        let mut link: Vec<(u32, u32, u32)> = Vec::with_capacity(faces.len());
        for &f in faces {
            let Some(&[a, b, c]) = triangles.get(f as usize) else {
                return Err(ModelError::Index { face: f });
            };
            let (x, y) = if a == v {
                (b, c)
            } else if b == v {
                (c, a)
            } else {
                (a, b)
            };
            link.push((x, y, f));
        }
        let Some(&(_, _, first)) = link.first() else {
            continue;
        };
        link.sort_unstable();
        // Start where no link edge comes in, if anywhere: a path's start.
        let start = link
            .iter()
            .find(|(x, _, _)| !link.iter().any(|(_, y, _)| y == x))
            .or_else(|| link.first())
            .map(|(x, _, _)| *x);
        let mut at = start;
        let mut walked = 0_usize;
        while let Some(x) = at {
            let Ok(k) = link.binary_search_by(|(p, _, _)| p.cmp(&x)) else {
                break;
            };
            walked = walked.saturating_add(1);
            at = link.get(k).map(|(_, y, _)| *y).filter(|y| Some(*y) != start);
            if walked > link.len() {
                break;
            }
        }
        if walked != link.len() {
            return Err(ModelError::NotSurface { face: first });
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "tests"
)]
mod tests {
    use super::*;
    use geom_run::Budget;

    /// A 3 × 3 sheet of cells of side 100 on z = 0: vertex 5 is inside.
    fn sheet() -> Surface {
        let mut points = Vec::new();
        for j in 0..4_i64 {
            for i in 0..4_i64 {
                points.push(P3::new(i * 100, j * 100, 0).unwrap());
            }
        }
        let at = |i: u32, j: u32| j * 4 + i;
        let mut triangles = Vec::new();
        for j in 0..3 {
            for i in 0..3 {
                triangles.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
                triangles.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
            }
        }
        let mut meter = WorkMeter::new(Budget::of_work(1 << 30));
        Surface::new(&points, &triangles, 4, &Exec::serial(), &mut meter).unwrap()
    }

    /// Everything a patch may change.
    type State = (Vec<P3>, Vec<[u32; 3]>, Vec<bool>, Vec<Vec<u32>>, usize);

    /// Everything a patch may change, now.
    fn state(s: &Surface) -> State {
        (
            s.points.clone(),
            s.faces.clone(),
            s.alive.clone(),
            s.around.clone(),
            s.live,
        )
    }

    #[test]
    fn a_refused_operation_leaves_the_surface_as_it_was() {
        let mut s = sheet();
        let mut meter = WorkMeter::new(Budget::of_work(1 << 30));
        let before = state(&s);
        // Lifted 10 steps, past ε = 4: the input is no longer covered.
        let lift = s.shift(5, P3::new(100, 100, 10).unwrap()).unwrap();
        assert!(!s.attempt(&lift, &mut meter).unwrap());
        assert_eq!(state(&s), before);
        // Splitting at a point off the edge by 10 steps: refused, the new vertex gone again.
        let split = s.split(5, 6, P3::new(150, 100, 10).unwrap()).unwrap();
        assert!(!s.attempt(&split, &mut meter).unwrap());
        assert_eq!(state(&s), before);
        // Collapsing a corner of the sheet into the middle: the corner's input is uncovered.
        let corner = s.collapse(5, 0, P3::new(100, 100, 0).unwrap()).unwrap();
        assert!(!s.attempt(&corner, &mut meter).unwrap());
        assert_eq!(state(&s), before);
        assert_eq!(s.finish().unwrap().operations(), (0, 3));
    }

    #[test]
    fn kept_operations_keep_the_surface_whole() {
        let mut s = sheet();
        let mut meter = WorkMeter::new(Budget::of_work(1 << 30));
        // Within the plane: kept.
        let slide = s.shift(5, P3::new(130, 90, 0).unwrap()).unwrap();
        assert!(s.attempt(&slide, &mut meter).unwrap());
        let split = s.split(5, 6, P3::new(150, 100, 0).unwrap()).unwrap();
        assert!(s.attempt(&split, &mut meter).unwrap());
        assert_eq!(s.live(), 20);
        let merge = s.collapse(5, 16, P3::new(130, 90, 0).unwrap()).unwrap();
        assert!(s.attempt(&merge, &mut meter).unwrap());
        assert_eq!(s.live(), 18);
        // Every list holds live triangles only, and each live triangle is in its corners' lists.
        for (v, list) in s.around.iter().enumerate() {
            for f in list {
                assert!(s.face(*f).unwrap().contains(&(v as u32)));
            }
        }
        let flip = s.flip(5, 9).or_else(|| s.flip(5, 10)).unwrap();
        assert!(s.attempt(&flip, &mut meter).unwrap());
        let out = s.finish().unwrap();
        assert_eq!(out.triangles().len(), 18);
        assert_eq!(out.points().len(), 16);
    }
}
