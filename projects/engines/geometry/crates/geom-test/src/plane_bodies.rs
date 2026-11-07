//! Plane-based bodies for the checkers to stand on (chapters 8.3 and 28.4; piece G135): polyhedra
//! with three faces at every vertex, whose planes can be moved a step off their grid points, so
//! that every vertex is the meeting of three planes at a place no grid point holds.
//!
//! What it does: a [`Spec`] is a polyhedron by its corners and, for each solid region, its shells,
//! faces and loops. [`meeting`] builds it with every vertex the meeting of its three faces' planes,
//! each plane through three corners of its face moved by a [`Jog`], and [`grid`] with every vertex
//! a grid point. [`CORPUS`] holds a box, an L, a wall with a window, two lumps, a cavity and a lump
//! floating in a cavity; [`scaled_jog`] picks moves for a seed.
//!
//! Guarantees:
//!
//! - **Valid by construction.** A move of a step or so on shapes hundreds of steps across changes no
//!   shape, so every body of the corpus is a valid body, by geometry rather than by the checker.
//! - **Deterministic.** A body is a function of its spec and its moves.
//! - **Refuses what it cannot build.** A vertex without exactly three faces has no one meeting:
//!   [`meeting`] returns `None`.
//!
//! Method: the bodies of chapter 11.3 and 19.2 (`projects/engines/geometry/architecture/11-booleans.md`):
//! each vertex the meeting of three of the body's planes, each edge's line on two.
//!
//! Cost: linear in the rows, with a quadratic search for each vertex's faces and each edge. No
//! recursion.

#![allow(
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    reason = "a test crate's fixtures: points by their letters, and whole numbers far below 2^53"
)]
#![allow(
    clippy::arithmetic_side_effects,
    reason = "test fixtures: coordinates in the hundreds and rows in the dozens"
)]

use geom_curve::{CurveKind, Line};
use geom_math::Box3;
use geom_num::P3;
use geom_num::plane::{Plane as GridPlane, Tpi};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};
use geom_topo::{
    Body, BodyKind, Builder, Edge, Face, Fin, Ix, Loop, PointKind, Region, RegionKind, Sense,
    Shell, Vertex,
};

/// One face: its loops of corner numbers, counter-clockwise seen from outside, holes clockwise.
pub type FaceSpec<'a> = &'a [&'a [u32]];
/// One shell: its faces.
pub type ShellSpec<'a> = &'a [FaceSpec<'a>];
/// How far anchor `.1` of face `.0` moves.
pub type Jog<'a> = &'a dyn Fn(usize, usize) -> [i64; 3];

/// A polyhedron: its corners, and for each solid region its shells.
#[derive(Clone, Copy, Debug)]
pub struct Spec<'a> {
    /// A name for reports.
    pub name: &'a str,
    /// The corners.
    pub points: &'a [[i64; 3]],
    /// Each solid region's shells.
    pub regions: &'a [&'a [ShellSpec<'a>]],
}

/// The corners of a box from `o`, of size `s`: bit 0 is x, bit 1 y, bit 2 z.
const fn corners(o: [i64; 3], s: [i64; 3]) -> [[i64; 3]; 8] {
    let [x, y, z] = o;
    let [a, b, c] = s;
    [
        [x, y, z],
        [x + a, y, z],
        [x, y + b, z],
        [x + a, y + b, z],
        [x, y, z + c],
        [x + a, y, z + c],
        [x, y + b, z + c],
        [x + a, y + b, z + c],
    ]
}

/// The faces of a box from corner `$n`, counter-clockwise from outside.
macro_rules! box_faces {
    ($n:expr) => {
        [
            &[&[$n, $n + 2, $n + 3, $n + 1]],
            &[&[$n + 4, $n + 5, $n + 7, $n + 6]],
            &[&[$n, $n + 1, $n + 5, $n + 4]],
            &[&[$n + 2, $n + 6, $n + 7, $n + 3]],
            &[&[$n, $n + 4, $n + 6, $n + 2]],
            &[&[$n + 1, $n + 3, $n + 7, $n + 5]],
        ]
    };
}

/// The same box inside out, a cavity's shell.
macro_rules! cavity_faces {
    ($n:expr) => {
        [
            &[&[$n, $n + 1, $n + 3, $n + 2]],
            &[&[$n + 4, $n + 6, $n + 7, $n + 5]],
            &[&[$n, $n + 4, $n + 5, $n + 1]],
            &[&[$n + 2, $n + 3, $n + 7, $n + 6]],
            &[&[$n, $n + 2, $n + 6, $n + 4]],
            &[&[$n + 1, $n + 5, $n + 7, $n + 3]],
        ]
    };
}

const BOX_POINTS: [[i64; 3]; 8] = corners([0, 0, 0], [100, 100, 100]);
const BOX_SHELL: [FaceSpec<'static>; 6] = box_faces!(0);

const ELL_POINTS: [[i64; 3]; 12] = [
    [0, 0, 0],
    [200, 0, 0],
    [200, 100, 0],
    [100, 100, 0],
    [100, 200, 0],
    [0, 200, 0],
    [0, 0, 100],
    [200, 0, 100],
    [200, 100, 100],
    [100, 100, 100],
    [100, 200, 100],
    [0, 200, 100],
];
const ELL_SHELL: [FaceSpec<'static>; 8] = [
    &[&[0, 5, 4, 3, 2, 1]],
    &[&[6, 7, 8, 9, 10, 11]],
    &[&[0, 1, 7, 6]],
    &[&[1, 2, 8, 7]],
    &[&[2, 3, 9, 8]],
    &[&[3, 4, 10, 9]],
    &[&[4, 5, 11, 10]],
    &[&[5, 0, 6, 11]],
];

const WALL_POINTS: [[i64; 3]; 16] = [
    [0, 0, 0],
    [400, 0, 0],
    [0, 0, 300],
    [400, 0, 300],
    [0, 50, 0],
    [400, 50, 0],
    [0, 50, 300],
    [400, 50, 300],
    [100, 0, 100],
    [300, 0, 100],
    [100, 0, 200],
    [300, 0, 200],
    [100, 50, 100],
    [300, 50, 100],
    [100, 50, 200],
    [300, 50, 200],
];
const WALL_SHELL: [FaceSpec<'static>; 10] = [
    &[&[0, 1, 3, 2], &[8, 10, 11, 9]],
    &[&[4, 6, 7, 5], &[12, 13, 15, 14]],
    &[&[0, 4, 5, 1]],
    &[&[2, 3, 7, 6]],
    &[&[0, 2, 6, 4]],
    &[&[1, 5, 7, 3]],
    &[&[8, 9, 13, 12]],
    &[&[10, 14, 15, 11]],
    &[&[8, 12, 14, 10]],
    &[&[9, 11, 15, 13]],
];

/// Two arrays of eight corners as one of sixteen.
const fn two(a: [[i64; 3]; 8], b: [[i64; 3]; 8]) -> [[i64; 3]; 16] {
    let [a0, a1, a2, a3, a4, a5, a6, a7] = a;
    let [b0, b1, b2, b3, b4, b5, b6, b7] = b;
    [
        a0, a1, a2, a3, a4, a5, a6, a7, b0, b1, b2, b3, b4, b5, b6, b7,
    ]
}

const TWO_POINTS: [[i64; 3]; 16] = two(
    corners([0, 0, 0], [100, 100, 100]),
    corners([300, 0, 0], [100, 100, 100]),
);
const SECOND_SHELL: [FaceSpec<'static>; 6] = box_faces!(8);
const CAVITY_POINTS: [[i64; 3]; 16] = two(
    corners([0, 0, 0], [900, 900, 900]),
    corners([100, 100, 100], [700, 700, 700]),
);
const CAVITY_SHELL: [FaceSpec<'static>; 6] = cavity_faces!(8);
const FLOATING_SHELL: [FaceSpec<'static>; 6] = box_faces!(16);

/// A box with a cavity, and a lump floating in the cavity.
const NEST_POINTS: [[i64; 3]; 24] = {
    let (a, b, c) = (
        corners([0, 0, 0], [900, 900, 900]),
        corners([100, 100, 100], [700, 700, 700]),
        corners([300, 300, 300], [300, 300, 300]),
    );
    let [a0, a1, a2, a3, a4, a5, a6, a7] = a;
    let [b0, b1, b2, b3, b4, b5, b6, b7] = b;
    let [c0, c1, c2, c3, c4, c5, c6, c7] = c;
    [
        a0, a1, a2, a3, a4, a5, a6, a7, b0, b1, b2, b3, b4, b5, b6, b7, c0, c1, c2, c3, c4, c5, c6,
        c7,
    ]
};

/// The valid bodies.
pub const CORPUS: [Spec<'static>; 6] = [
    Spec {
        name: "box",
        points: &BOX_POINTS,
        regions: &[&[&BOX_SHELL]],
    },
    Spec {
        name: "L",
        points: &ELL_POINTS,
        regions: &[&[&ELL_SHELL]],
    },
    Spec {
        name: "wall with a window",
        points: &WALL_POINTS,
        regions: &[&[&WALL_SHELL]],
    },
    Spec {
        name: "two lumps",
        points: &TWO_POINTS,
        regions: &[&[&BOX_SHELL], &[&SECOND_SHELL]],
    },
    Spec {
        name: "a box with a cavity",
        points: &CAVITY_POINTS,
        regions: &[&[&BOX_SHELL, &CAVITY_SHELL]],
    },
    Spec {
        name: "a lump in a cavity",
        points: &NEST_POINTS,
        regions: &[&[&BOX_SHELL, &CAVITY_SHELL], &[&FLOATING_SHELL]],
    },
];

/// No move at all.
#[must_use]
pub const fn still(_: usize, _: usize) -> [i64; 3] {
    [0; 3]
}

/// A step or so each way, a different move at every anchor of every face, from `seed`
/// (`SplitMix64`'s finaliser mixes the three together).
pub fn scaled_jog(seed: u64) -> impl Fn(usize, usize) -> [i64; 3] {
    move |face, anchor| {
        let mix = |k: u64| {
            let z = seed
                .wrapping_add((face as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15))
                .wrapping_add((anchor as u64 + 1).wrapping_mul(0xBF58_476D_1CE4_E5B9))
                .wrapping_add(k.wrapping_mul(0x94D0_49BB_1331_11EB));
            let z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            i64::try_from((z ^ (z >> 27)) % 3).unwrap_or(1) - 1
        };
        [mix(0), mix(1), mix(2)]
    }
}

/// `a × b`.
const fn cross(a: [i128; 3], b: [i128; 3]) -> [i128; 3] {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}

/// `a − b`.
const fn minus(a: [i128; 3], b: [i128; 3]) -> [i128; 3] {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    [ax - bx, ay - by, az - bz]
}

/// Three corners of `ring` that turn the way it runs, by the ring's area vector: a corner of a loop
/// that is not convex can turn the other way.
fn anchors(points: &[[i64; 3]], ring: &[u32]) -> Option<[[i64; 3]; 3]> {
    let at = |v: u32| points.get(v as usize).map(|p| p.map(i128::from));
    let n = ring.len();
    let mut area = [0_i128; 3];
    let rows = |k: usize| ring.get(k % n).copied();
    for k in 0..n {
        let c = cross(at(rows(k)?)?, at(rows(k + 1)?)?);
        let ([x, y, z], [cx, cy, cz]) = (area, c);
        area = [x + cx, y + cy, z + cz];
    }
    (0..n)
        .find_map(|k| {
            let (a, b, c) = (rows(k)?, rows(k + 1)?, rows(k + 2)?);
            let turn = cross(minus(at(b)?, at(a)?), minus(at(c)?, at(a)?));
            let ([tx, ty, tz], [ax, ay, az]) = (turn, area);
            let along = tx * ax + ty * ay + tz * az;
            (along > 0).then(|| [a, b, c].map(|v| points.get(v as usize).copied()))
        })
        .and_then(|[a, b, c]| Some([a?, b?, c?]))
}

/// A body's rows as they are made.
#[derive(Default)]
struct Rows {
    regions: Vec<Region>,
    shells: Vec<Shell>,
    faces: Vec<Face>,
    loops: Vec<Loop>,
    fins: Vec<Fin>,
    edges: Vec<Edge>,
    vertices: Vec<Vertex>,
    surfaces: Vec<SurfaceKind>,
    curves: Vec<CurveKind<3>>,
    points: Vec<PointKind>,
    planes: Vec<GridPlane>,
    /// Where each vertex lies or rounds to.
    places: Vec<[i64; 3]>,
    /// Each edge's two vertex numbers, low first.
    pairs: Vec<(u32, u32)>,
}

/// The index of row `row`.
fn ix<K>(row: usize) -> Option<Ix<K>> {
    Ix::new(u32::try_from(row).ok()?)
}

impl Rows {
    /// The edge between two vertices, made the first time they meet, and the fin's sense along it.
    fn edge(&mut self, a: u32, b: u32) -> Option<(usize, Sense)> {
        let key = (a.min(b), a.max(b));
        let sense = if a < b {
            Sense::Forward
        } else {
            Sense::Reversed
        };
        if let Some(e) = self.pairs.iter().position(|p| *p == key) {
            return Some((e, sense));
        }
        let (from, to) = (
            *self.places.get(key.0 as usize)?,
            *self.places.get(key.1 as usize)?,
        );
        let ([tx, ty, tz], [fx, fy, fz]) = (to, from);
        let direction = [(tx - fx) as f64, (ty - fy) as f64, (tz - fz) as f64];
        let direction = if direction == [0.0; 3] {
            [1.0, 0.0, 0.0]
        } else {
            direction
        };
        let line = Line::new(from.map(|x| x as f64), direction, (0.0, 1.0)).ok()?;
        self.curves.push(CurveKind::Line(line));
        self.pairs.push(key);
        self.edges.push(Edge {
            first_fin: None,
            start: Some(ix(key.0 as usize)?),
            end: Some(ix(key.1 as usize)?),
            curve: ix(self.curves.len() - 1)?,
        });
        Some((self.edges.len() - 1, sense))
    }

    /// The face of `loops` on the plane through `on`, in shell `shell`, boxed a step wide of its
    /// corners.
    fn face(&mut self, shell: usize, loops: &[&[u32]], on: [[i64; 3]; 3]) -> Option<()> {
        let f = self.faces.len();
        let [o, a, b] = on.map(|p| p.map(|x| x as f64));
        let [ox, oy, oz] = o;
        let span = |[x, y, z]: [f64; 3]| [x - ox, y - oy, z - oz];
        let flat = Plane::new(o, span(a), span(b), ((0.0, 1.0), (0.0, 1.0))).ok()?;
        self.surfaces.push(SurfaceKind::Plane(flat));
        let mut low = [i64::MAX; 3];
        let mut high = [i64::MIN; 3];
        for v in loops.iter().copied().flatten() {
            let at = self.places.get(*v as usize)?;
            for (((lo, hi), at), _) in low.iter_mut().zip(&mut high).zip(at).zip(0..3) {
                *lo = (*lo).min(at - 1);
                *hi = (*hi).max(at + 1);
            }
        }
        let point = |[x, y, z]: [i64; 3]| P3::new(x, y, z).ok();
        self.faces.push(Face {
            shell: ix(shell)?,
            next: None,
            surface: ix(self.surfaces.len() - 1)?,
            sense: Sense::Forward,
            first_loop: ix(self.loops.len()),
            bound: Box3::new(point(low)?, point(high)?).ok()?,
        });
        for (k, ring) in loops.iter().enumerate() {
            let first = self.fins.len();
            let n = ring.len();
            let next = (k + 1 < loops.len()).then_some(self.loops.len() + 1);
            self.loops.push(Loop {
                face: ix(f)?,
                next: next.and_then(ix),
                first_fin: ix(first),
            });
            for (at, &a) in ring.iter().enumerate() {
                let b = *ring.get((at + 1) % n)?;
                let (e, sense) = self.edge(a, b)?;
                let fin = first + at;
                let radial = match self.edges.get(e)?.first_fin {
                    None => {
                        self.edges.get_mut(e)?.first_fin = ix(fin);
                        ix(fin)?
                    }
                    Some(head) => {
                        let mut last = head;
                        while self.fins.get(last.to_usize())?.radial != head {
                            last = self.fins.get(last.to_usize())?.radial;
                        }
                        self.fins.get_mut(last.to_usize())?.radial = ix(fin)?;
                        head
                    }
                };
                self.fins.push(Fin {
                    loop_: ix(self.loops.len() - 1)?,
                    next: ix(first + (at + 1) % n)?,
                    prev: ix(first + (at + n - 1) % n)?,
                    edge: ix(e)?,
                    sense,
                    radial,
                    pcurve: None,
                });
            }
        }
        Some(())
    }

    /// The shells, faces and lists of `spec`, on the planes `on`, one per face in order.
    fn shells(&mut self, spec: Spec<'_>, on: &[[[i64; 3]; 3]]) -> Option<()> {
        self.regions.push(Region {
            kind: RegionKind::Void,
            first_shell: None,
        });
        let mut face = 0;
        for shells in spec.regions {
            let region = self.regions.len();
            self.regions.push(Region {
                kind: RegionKind::Solid,
                first_shell: ix(self.shells.len()),
            });
            for (k, shell) in shells.iter().enumerate() {
                let s = self.shells.len();
                self.shells.push(Shell {
                    region: ix(region)?,
                    next: (k + 1 < shells.len()).then_some(s + 1).and_then(ix),
                    first_face: ix(self.faces.len()),
                });
                for (at, loops) in shell.iter().enumerate() {
                    let f = self.faces.len();
                    self.face(s, loops, *on.get(face)?)?;
                    self.faces.get_mut(f)?.next =
                        (at + 1 < shell.len()).then_some(f + 1).and_then(ix);
                    face += 1;
                }
            }
        }
        Some(())
    }

    /// The body these rows make.
    fn body(self) -> Option<Body> {
        let mut b = Builder::new();
        let a = &mut b.arenas;
        a.regions.extend(&self.regions).ok()?;
        a.shells.extend(&self.shells).ok()?;
        a.faces.extend(&self.faces).ok()?;
        a.loops.extend(&self.loops).ok()?;
        a.fins.extend(&self.fins).ok()?;
        a.edges.extend(&self.edges).ok()?;
        a.vertices.extend(&self.vertices).ok()?;
        a.surfaces.extend(self.surfaces).ok()?;
        a.curves.extend(self.curves).ok()?;
        a.points.extend(self.points).ok()?;
        a.planes.extend(self.planes).ok()?;
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        b.publish(BodyKind::Solid, ix(0)?, &mut meter).ok()
    }
}

/// Every face of `spec`, in order.
fn faces(spec: Spec<'_>) -> Vec<FaceSpec<'_>> {
    spec.regions
        .iter()
        .flat_map(|shells| shells.iter())
        .flat_map(|shell| shell.iter().copied())
        .collect()
}

/// Each face's anchors, moved by `jog`.
fn moved_anchors(spec: Spec<'_>, jog: Jog<'_>) -> Option<Vec<[[i64; 3]; 3]>> {
    faces(spec)
        .iter()
        .enumerate()
        .map(|(k, loops)| {
            let [a, b, c] = anchors(spec.points, loops.first()?)?;
            let moved = |[x, y, z]: [i64; 3], n: usize| {
                let [dx, dy, dz] = jog(k, n);
                [x + dx, y + dy, z + dz]
            };
            Some([moved(a, 0), moved(b, 1), moved(c, 2)])
        })
        .collect()
}

/// `spec`'s body, every vertex the meeting of its three faces' planes, each plane through its
/// face's anchors moved by `jog`. `None` for a vertex without three faces, or planes that do not
/// meet in a point the grid holds.
#[must_use]
pub fn meeting(spec: Spec<'_>, jog: Jog<'_>) -> Option<Body> {
    let on = moved_anchors(spec, jog)?;
    let mut r = Rows::default();
    let point = |[x, y, z]: [i64; 3]| P3::new(x, y, z).ok();
    for [a, b, c] in &on {
        r.planes
            .push(GridPlane::through(point(*a)?, point(*b)?, point(*c)?).ok()?);
    }
    let list = faces(spec);
    for v in 0..u32::try_from(spec.points.len()).ok()? {
        let on: Vec<usize> = list
            .iter()
            .enumerate()
            .filter(|(_, loops)| loops.iter().any(|ring| ring.contains(&v)))
            .map(|(k, _)| k)
            .collect();
        let [first, second, third] = on.as_slice() else {
            return None;
        };
        let (p, q, s) = (
            *r.planes.get(*first)?,
            *r.planes.get(*second)?,
            *r.planes.get(*third)?,
        );
        let at = Tpi::new(p, q, s).ok()?.to_grid().ok()?.value();
        r.places.push([at.x.get(), at.y.get(), at.z.get()]);
        r.points
            .push(PointKind::Meet([ix(*first)?, ix(*second)?, ix(*third)?]));
        r.vertices.push(Vertex {
            point: ix(r.points.len() - 1)?,
        });
    }
    r.shells(spec, &on)?;
    r.body()
}

/// `spec`'s body with every vertex a grid point.
#[must_use]
pub fn grid(spec: Spec<'_>) -> Option<Body> {
    let on = moved_anchors(spec, &still)?;
    let mut r = Rows::default();
    for p in spec.points {
        r.places.push(*p);
        let [x, y, z] = *p;
        r.points.push(PointKind::Grid(P3::new(x, y, z).ok()?));
        r.vertices.push(Vertex {
            point: ix(r.points.len() - 1)?,
        });
    }
    r.shells(spec, &on)?;
    r.body()
}
