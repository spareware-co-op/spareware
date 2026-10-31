//! Exact convex hulls of grid points (19.5).
//!
//! What it does: [`Hull::of`] finds the convex hull of a set of grid points: its corners, and the
//! ring of corners round each of its faces, outward. [`Hull::mesh`] makes it a closed exact mesh.
//!
//! Guarantees:
//!
//! - **Exact.** Every decision is an exact orientation test. No point is near a face: it is above,
//!   on or below it, and a hull is the same at every scale.
//! - **Coplanar points are no special case.** Ties break by symbolic perturbation in the points'
//!   sorted order, so the search always sees a hull of triangles. Triangles on one plane then merge
//!   into one face, and points on an edge or inside a face drop out. A face's corners are always
//!   corners of the hull: no three in a ring are on one line.
//! - **The order of the input does not show.** Points are sorted and repeats dropped first, so the
//!   same set of points gives the same hull, face for face.
//! - **Flat sets are answers too.** Points on a plane give one face, on a line its two ends, and one
//!   point itself. Only a solid hull makes a mesh.
//!
//! Method: quickhull (Barber, Dobkin and Huhdanpaa, 1996,
//! <https://doi.org/10.1145/235815.235821>), with Edelsbrunner and Mücke's Simulation of Simplicity
//! (<https://doi.org/10.1145/77635.77639>) in place of its tolerances (19.5, 6.2). The farthest
//! point is only a choice of which to take next, so a rough plane in coordinates cut to 21 bits
//! serves; the exact tests decide everything else. A face's ring is Andrew's monotone chain
//! (<https://doi.org/10.1016/0020-0190(79)90072-3>) in the two axes its normal leans on least.
//!
//! Cost: about O(n log n) orientation tests for points in general position, O(n²) at worst, charged
//! to the meter one by one, then a sort of the triangles by plane.

#![allow(
    clippy::many_single_char_names,
    reason = "a face's corners and a point's coordinates, as in the predicates"
)]

use alloc::vec::Vec;
use core::cmp::Reverse;
use core::fmt;

use geom_num::grid::{P2, P3};
use geom_num::perturb::orient3d;
use geom_num::plane::Plane;
use geom_num::predicate::orient2d;
use geom_num::{Exactness, NumError, Sign};
use geom_poly::plane::canonical;
use geom_run::{Exec, Stopped, WorkMeter};

use crate::exact::build::Corner;
use crate::exact::build::{Polygons, Source, build};
use crate::exact::{ExactMesh, MeshError, PropTable};

/// How many bits of a coordinate the rough planes keep.
const ROUGH_BITS: u32 = 20;

/// Why a hull could not be found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HullError {
    /// The numbers layer refused.
    Num(NumError),
    /// The meter stopped the run.
    Stopped(Stopped),
    /// More points than 32-bit indices can name.
    Full,
    /// Two faces lost their link: a defect here, never the input's doing.
    Defect,
}

impl fmt::Display for HullError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HullError::Num(e) => write!(f, "a hull's test failed: {e:?}"),
            HullError::Stopped(_) => f.write_str("the meter stopped the hull"),
            HullError::Full => f.write_str("more points than 32-bit indices can name"),
            HullError::Defect => f.write_str("a hull lost a link between its faces"),
        }
    }
}

impl core::error::Error for HullError {}

impl From<NumError> for HullError {
    fn from(e: NumError) -> Self {
        HullError::Num(e)
    }
}

impl From<Stopped> for HullError {
    fn from(e: Stopped) -> Self {
        HullError::Stopped(e)
    }
}

/// A triangle of the search.
struct Face {
    /// Its corners, turning counter-clockwise seen from outside.
    corners: [u32; 3],
    /// The face across the edge from each corner to the next.
    across: [u32; 3],
    /// A rough normal.
    normal: [i128; 3],
    /// The points outside it that no other face holds.
    outside: Vec<u32>,
    alive: bool,
}

/// Whether a face was seen from the current point, by the number of the round that looked.
#[derive(Clone, Copy, Default)]
struct Mark {
    round: u32,
    lit: bool,
}

/// One edge the visible faces end at: from `from` to `to`, with `beyond` the face past it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Rim {
    from: u32,
    to: u32,
    beyond: u32,
}

/// The search: sorted points, their rough copies, and the triangles so far.
struct Quick<'p> {
    points: &'p [P3],
    rough: Vec<[i64; 3]>,
    faces: Vec<Face>,
}

/// The index of a count.
fn index(n: usize) -> Result<u32, HullError> {
    u32::try_from(n).map_err(|_| HullError::Full)
}

/// `b − a`, each part below 2^22.
fn difference(a: [i64; 3], b: [i64; 3]) -> [i128; 3] {
    let [x, y, z] = a;
    let [u, v, w] = b;
    let gap = |p: i64, q: i64| i128::from(q.wrapping_sub(p));
    [gap(x, u), gap(y, v), gap(z, w)]
}

/// u × v of rough differences, each part below 2^45.
fn cross(u: [i128; 3], v: [i128; 3]) -> [i128; 3] {
    let [a, b, c] = u;
    let [x, y, z] = v;
    let term =
        |p: i128, q: i128, r: i128, s: i128| p.wrapping_mul(q).wrapping_sub(r.wrapping_mul(s));
    [term(b, z, c, y), term(c, x, a, z), term(a, y, b, x)]
}

/// u · v of rough values, below 2^92.
fn dot(u: [i128; 3], v: [i128; 3]) -> i128 {
    let [a, b, c] = u;
    let [x, y, z] = v;
    a.wrapping_mul(x)
        .wrapping_add(b.wrapping_mul(y))
        .wrapping_add(c.wrapping_mul(z))
}

/// `points` cut to [`ROUGH_BITS`] bits of their largest extent, a sign bit aside.
fn rough(points: &[P3]) -> Vec<[i64; 3]> {
    let parts = |p: &P3| [p.x.get(), p.y.get(), p.z.get()];
    let mut span = 0_u64;
    for axis in 0..3_usize {
        let along = |p: &P3| parts(p).get(axis).copied().unwrap_or(0);
        let low = points.iter().map(along).min().unwrap_or(0);
        let high = points.iter().map(along).max().unwrap_or(0);
        span = span.max(high.abs_diff(low));
    }
    let shift = u64::BITS
        .saturating_sub(span.leading_zeros())
        .saturating_sub(ROUGH_BITS);
    let cut = |c: i64| c.wrapping_shr(shift);
    points
        .iter()
        .map(|p| [cut(p.x.get()), cut(p.y.get()), cut(p.z.get())])
        .collect()
}

impl Quick<'_> {
    fn face(&self, f: u32) -> Result<&Face, HullError> {
        self.faces.get(f as usize).ok_or(HullError::Defect)
    }

    fn face_mut(&mut self, f: u32) -> Result<&mut Face, HullError> {
        self.faces.get_mut(f as usize).ok_or(HullError::Defect)
    }

    fn rough_at(&self, i: u32) -> Result<[i64; 3], HullError> {
        self.rough.get(i as usize).copied().ok_or(HullError::Defect)
    }

    /// Point `i`, keyed by its place in the sort.
    fn keyed(&self, i: u32) -> Result<(P3, u64), HullError> {
        let p = self.points.get(i as usize).ok_or(HullError::Defect)?;
        Ok((*p, u64::from(i)))
    }

    /// Whether point `p` is outside the plane of face `f`, after perturbation.
    fn sees(&self, f: u32, p: u32, meter: &mut WorkMeter<'_>) -> Result<bool, HullError> {
        meter.charge(1)?;
        let [a, b, c] = self.face(f)?.corners;
        let sign = orient3d(
            self.keyed(a)?,
            self.keyed(b)?,
            self.keyed(c)?,
            self.keyed(p)?,
        )?;
        Ok(sign == Sign::Positive)
    }

    /// A new face, lit by nothing, with `across` its neighbours where they are known.
    fn add(&mut self, corners: [u32; 3], across: [u32; 3]) -> Result<u32, HullError> {
        let [a, b, c] = corners;
        let (p, q, r) = (self.rough_at(a)?, self.rough_at(b)?, self.rough_at(c)?);
        let normal = cross(difference(p, q), difference(p, r));
        let at = index(self.faces.len())?;
        self.faces.push(Face {
            corners,
            across,
            normal,
            outside: Vec::new(),
            alive: true,
        });
        Ok(at)
    }

    /// Four points to start from, far apart where that is easy to see. Any four distinct points
    /// make a solid under perturbation. There are at least four.
    fn simplex(&self, meter: &mut WorkMeter<'_>) -> Result<[u32; 4], HullError> {
        let last = index(self.points.len())?.saturating_sub(1);
        let (a, b) = (0, last);
        let (p, q) = (self.rough_at(a)?, self.rough_at(b)?);
        let line = difference(p, q);
        let off_line = |i: u32| -> Option<(i128, Reverse<u32>)> {
            let at = self.rough.get(i as usize).copied()?;
            let n = cross(line, difference(p, at));
            Some((dot(n, n), Reverse(i)))
        };
        let inner = 1..last;
        let c = inner
            .clone()
            .filter_map(off_line)
            .max()
            .map(|(_, Reverse(i))| i)
            .ok_or(HullError::Defect)?;
        let normal = cross(line, difference(p, self.rough_at(c)?));
        let off_plane = |i: u32| -> Option<(i128, Reverse<u32>)> {
            let at = self.rough.get(i as usize).copied()?;
            Some((dot(normal, difference(p, at)).wrapping_abs(), Reverse(i)))
        };
        let d = inner
            .filter(|i| *i != c)
            .filter_map(off_plane)
            .max()
            .map(|(_, Reverse(i))| i)
            .ok_or(HullError::Defect)?;
        meter.charge(1)?;
        let sign = orient3d(
            self.keyed(a)?,
            self.keyed(b)?,
            self.keyed(c)?,
            self.keyed(d)?,
        )?;
        Ok(if sign == Sign::Negative {
            [a, b, c, d]
        } else {
            [a, c, b, d]
        })
    }

    /// The tetrahedron `corners`, whose fourth is behind its first three, as four faces, with the
    /// other points outside the first face that sees them.
    fn start(&mut self, corners: [u32; 4], meter: &mut WorkMeter<'_>) -> Result<(), HullError> {
        let [a, b, c, d] = corners;
        self.add([a, b, c], [1, 2, 3])?;
        self.add([a, d, b], [3, 2, 0])?;
        self.add([b, d, c], [1, 3, 0])?;
        self.add([c, d, a], [2, 1, 0])?;
        for p in 0..index(self.points.len())? {
            if corners.contains(&p) {
                continue;
            }
            self.give(p, 0..4, meter)?;
        }
        Ok(())
    }

    /// Point `p` to the first of `faces` that sees it, or to none: it is inside.
    fn give(
        &mut self,
        p: u32,
        faces: core::ops::Range<u32>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(), HullError> {
        for f in faces {
            if self.sees(f, p, meter)? {
                self.face_mut(f)?.outside.push(p);
                break;
            }
        }
        Ok(())
    }

    /// The point outside `f` farthest above it, by rough plane; `None` for a face with no work.
    fn farthest(&self, f: u32) -> Result<Option<u32>, HullError> {
        let face = self.face(f)?;
        let [a, _, _] = face.corners;
        if !face.alive {
            return Ok(None);
        }
        let base = self.rough_at(a)?;
        let mut best: Option<(i128, u32)> = None;
        for &p in &face.outside {
            let height = dot(face.normal, difference(base, self.rough_at(p)?));
            if best.is_none_or(|(most, _)| height > most) {
                best = Some((height, p));
            }
        }
        Ok(best.map(|(_, p)| p))
    }

    /// The faces `eye` sees, from `start`, which it sees, and the rim round them.
    fn lit(
        &self,
        start: u32,
        eye: u32,
        round: u32,
        marks: &mut Vec<Mark>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Vec<u32>, Vec<Rim>), HullError> {
        marks.resize(self.faces.len(), Mark::default());
        let mark = marks.get_mut(start as usize).ok_or(HullError::Defect)?;
        *mark = Mark { round, lit: true };
        let mut lit = alloc::vec![start];
        let mut rim = Vec::new();
        let mut at = 0;
        while let Some(&f) = lit.get(at) {
            at = at.saturating_add(1);
            let face = self.face(f)?;
            let [a, b, c] = face.corners;
            let [x, y, z] = face.across;
            for (from, to, beyond) in [(a, b, x), (b, c, y), (c, a, z)] {
                let slot = marks.get_mut(beyond as usize).ok_or(HullError::Defect)?;
                let bright = if slot.round == round {
                    slot.lit
                } else {
                    let bright = self.sees(beyond, eye, meter)?;
                    *slot = Mark { round, lit: bright };
                    if bright {
                        lit.push(beyond);
                    }
                    bright
                };
                if !bright {
                    rim.push(Rim { from, to, beyond });
                }
            }
        }
        Ok((lit, rim))
    }

    /// Replaces `lit` by a cone of faces from `rim` to `eye`, and gives their points out again.
    fn cone(
        &mut self,
        eye: u32,
        lit: &[u32],
        rim: &mut [Rim],
        pending: &mut Vec<u32>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(), HullError> {
        rim.sort_unstable();
        let first = index(self.faces.len())?;
        for edge in rim.iter() {
            let made = self.add([edge.from, edge.to, eye], [edge.beyond, 0, 0])?;
            let other = self.face_mut(edge.beyond)?;
            let [a, b, c] = other.corners;
            let [x, y, z] = &mut other.across;
            let back = (edge.to, edge.from);
            let slot = match back {
                pair if pair == (a, b) => x,
                pair if pair == (b, c) => y,
                pair if pair == (c, a) => z,
                _ => return Err(HullError::Defect),
            };
            *slot = made;
        }
        for (edge, made) in rim.iter().zip(first..) {
            let place = rim
                .binary_search_by_key(&edge.to, |e| e.from)
                .map_err(|_| HullError::Defect)?;
            let next = first.saturating_add(index(place)?);
            let [_, here, _] = &mut self.face_mut(made)?.across;
            *here = next;
            let [_, _, there] = &mut self.face_mut(next)?.across;
            *there = made;
        }
        let end = index(self.faces.len())?;
        let mut orphans = Vec::new();
        for &f in lit {
            let face = self.face_mut(f)?;
            face.alive = false;
            orphans.append(&mut face.outside);
        }
        for p in orphans {
            if p != eye {
                self.give(p, first..end, meter)?;
            }
        }
        for f in first..end {
            if !self.face(f)?.outside.is_empty() {
                pending.push(f);
            }
        }
        Ok(())
    }

    /// Runs the search: every point inside the faces, or a corner of them.
    fn run(&mut self, meter: &mut WorkMeter<'_>) -> Result<(), HullError> {
        let mut pending: Vec<u32> = (0..4).collect();
        let mut marks = Vec::new();
        let mut round = 0_u32;
        while let Some(start) = pending.pop() {
            let Some(eye) = self.farthest(start)? else {
                continue;
            };
            round = round.saturating_add(1);
            let (lit, mut rim) = self.lit(start, eye, round, &mut marks, meter)?;
            self.cone(eye, &lit, &mut rim, &mut pending, meter)?;
        }
        Ok(())
    }
}

/// The two axes a normal leans on least, in the order that makes them a right-handed turn about
/// the third, and whether the normal runs up that third axis.
fn plan_axes(n: [i128; 3]) -> ([usize; 2], bool) {
    let [x, y, z] = n;
    let (a, b, c) = (x.unsigned_abs(), y.unsigned_abs(), z.unsigned_abs());
    if a >= b && a >= c {
        ([1, 2], x > 0)
    } else if b >= c {
        ([2, 0], y > 0)
    } else {
        ([0, 1], z > 0)
    }
}

/// The ring round `corners`, which lie on a plane with outward normal `n`: counter-clockwise from
/// outside, with no three on a line.
fn ring(points: &[P3], mut corners: Vec<u32>, n: [i128; 3]) -> Result<Vec<u32>, HullError> {
    let ([i, j], up) = plan_axes(n);
    let plan = |c: u32| -> Result<P2, HullError> {
        let p = points.get(c as usize).ok_or(HullError::Defect)?;
        let parts = [p.x.get(), p.y.get(), p.z.get()];
        let (u, v) = (parts.get(i), parts.get(j));
        let (u, v) = u.zip(v).ok_or(HullError::Defect)?;
        Ok(P2::new(*u, *v)?)
    };
    let mut planned = Vec::with_capacity(corners.len());
    for c in corners.drain(..) {
        let p = plan(c)?;
        planned.push((p.x.get(), p.y.get(), c, p));
    }
    planned.sort_unstable_by_key(|&(x, y, c, _)| (x, y, c));
    planned.dedup_by_key(|&mut (_, _, c, _)| c);
    let chain = |walk: &mut dyn Iterator<Item = &(i64, i64, u32, P2)>| -> Result<Vec<(u32, P2)>, HullError> {
        let mut held: Vec<(u32, P2)> = Vec::new();
        for &(_, _, c, p) in walk {
            while let [.., (_, a), (_, b)] = held.as_slice() {
                if orient2d(*a, *b, p)? == Sign::Positive {
                    break;
                }
                held.pop();
            }
            held.push((c, p));
        }
        held.pop();
        Ok(held)
    };
    let mut out = chain(&mut planned.iter())?;
    out.append(&mut chain(&mut planned.iter().rev())?);
    let mut rung: Vec<u32> = out.into_iter().map(|(c, _)| c).collect();
    if !up {
        rung.reverse();
    }
    Ok(rung)
}

/// A triangle's plane, for the sort: its reduced coefficients, then which way it faces.
type Key = (([i128; 3], i128), bool);

/// The rings of the faces of the triangles `triangles`, one to a plane, in the planes' order.
/// Triangles on a line stand on an edge of a face, and add nothing.
fn merge(
    points: &[P3],
    triangles: &[[u32; 3]],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<(Key, Vec<u32>)>, HullError> {
    let at = |i: u32| points.get(i as usize).copied().ok_or(HullError::Defect);
    let mut keyed: Vec<(Key, [u32; 3])> = Vec::with_capacity(triangles.len());
    for t in triangles {
        meter.charge(1)?;
        let [a, b, c] = *t;
        match Plane::through(at(a)?, at(b)?, at(c)?) {
            Ok(plane) => {
                let reduced = canonical(&plane)?;
                let key = ((reduced.plane.n(), reduced.plane.d()), reduced.flipped);
                keyed.push((key, *t));
            }
            Err(NumError::Degenerate) => {}
            Err(e) => return Err(e.into()),
        }
    }
    keyed.sort_unstable();
    let mut out = Vec::new();
    let mut rest = keyed.as_slice();
    while let Some(((key, _), _)) = rest.split_first() {
        let same = rest.iter().take_while(|(k, _)| k == key).count();
        let (group, tail) = rest.split_at(same);
        rest = tail;
        let mut corners = Vec::with_capacity(same.saturating_mul(3));
        for (_, t) in group {
            let [a, b, c] = *t;
            corners.push(a);
            corners.push(b);
            corners.push(c);
        }
        let ((n, _), flipped) = *key;
        let outward = if flipped {
            n.map(i128::wrapping_neg)
        } else {
            n
        };
        out.push((*key, ring(points, corners, outward)?));
    }
    Ok(out)
}

/// The convex hull of some grid points: its corners in sorted order, and the ring of each face, in
/// numbers of those corners, in the order of the faces' planes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hull {
    points: Vec<P3>,
    faces: Vec<Vec<u32>>,
}

impl Hull {
    /// The hull of `points`.
    ///
    /// # Errors
    ///
    /// A [`HullError`]: a stopped meter, or more points than 32-bit indices can name.
    pub fn of(points: &[P3], meter: &mut WorkMeter<'_>) -> Result<Self, HullError> {
        let mut sorted: Vec<P3> = points.to_vec();
        sorted.sort_unstable_by_key(|p| (p.x.get(), p.y.get(), p.z.get()));
        sorted.dedup();
        index(sorted.len())?;
        let triangles: Vec<[u32; 3]> = match sorted.len() {
            0..=2 => Vec::new(),
            3 => alloc::vec![[0, 1, 2], [0, 2, 1]],
            _ => {
                let mut quick = Quick {
                    rough: rough(&sorted),
                    points: &sorted,
                    faces: Vec::new(),
                };
                let corners = quick.simplex(meter)?;
                quick.start(corners, meter)?;
                quick.run(meter)?;
                quick
                    .faces
                    .iter()
                    .filter(|f| f.alive)
                    .map(|f| f.corners)
                    .collect()
            }
        };
        let mut rings = merge(&sorted, &triangles, meter)?;
        if rings.len() < 4 {
            // Flat: the two sides of one polygon.
            rings.retain(|((_, flipped), _)| !*flipped);
        }
        if rings.is_empty() {
            let ends = [sorted.first(), sorted.last()];
            let mut points: Vec<P3> = ends.into_iter().flatten().copied().collect();
            points.dedup();
            return Ok(Self {
                points,
                faces: Vec::new(),
            });
        }
        let mut used: Vec<u32> = rings.iter().flat_map(|(_, r)| r.iter().copied()).collect();
        used.sort_unstable();
        used.dedup();
        let mut corners = Vec::with_capacity(used.len());
        for &u in &used {
            corners.push(*sorted.get(u as usize).ok_or(HullError::Defect)?);
        }
        let mut faces = Vec::with_capacity(rings.len());
        for (_, r) in rings {
            let mut numbered = Vec::with_capacity(r.len());
            for c in r {
                numbered.push(index(
                    used.binary_search(&c).map_err(|_| HullError::Defect)?,
                )?);
            }
            faces.push(numbered);
        }
        Ok(Self {
            points: corners,
            faces,
        })
    }

    /// The hull's corners, in sorted order.
    #[must_use]
    pub fn points(&self) -> &[P3] {
        &self.points
    }

    /// Each face's ring, counter-clockwise from outside, as numbers of [`Hull::points`]: one face
    /// for a flat hull, none for a line or a point.
    #[must_use]
    pub fn faces(&self) -> &[Vec<u32>] {
        &self.faces
    }

    /// Whether the hull has an inside: four faces at least.
    #[must_use]
    pub fn is_solid(&self) -> bool {
        self.faces.len() >= 4
    }

    /// The hull's faces as polygons of one run.
    #[must_use]
    pub fn polygons(&self, source: Source) -> Polygons {
        let mut corners = Vec::new();
        let mut loop_start = alloc::vec![0_u32];
        for face in &self.faces {
            corners.extend_from_slice(face);
            loop_start.push(u32::try_from(corners.len()).unwrap_or(u32::MAX));
        }
        let faces = u32::try_from(self.faces.len()).unwrap_or(u32::MAX);
        Polygons {
            vertices: self.points.iter().copied().map(Corner::Grid).collect(),
            corners,
            loop_start,
            face_loops: (0..=faces).collect(),
            face_run: alloc::vec![0; self.faces.len()],
            runs: alloc::vec![source],
            face_plane: Vec::new(),
            edge_plane: Vec::new(),
            corner_prop: Vec::new(),
            props: PropTable::default(),
            face_dev: Vec::new(),
            class: Exactness::Exact,
        }
    }

    /// The hull as a closed exact mesh.
    ///
    /// # Errors
    ///
    /// [`MeshError::NotClosed`] for a flat hull, and what [`build`] returns for a stopped meter.
    pub fn mesh(
        &self,
        source: Source,
        exec: &Exec<'_>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<ExactMesh, MeshError> {
        if !self.is_solid() {
            return Err(MeshError::NotClosed);
        }
        build(&self.polygons(source), exec, meter)
    }
}
