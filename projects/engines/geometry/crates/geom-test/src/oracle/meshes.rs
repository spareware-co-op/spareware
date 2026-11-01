//! Mesh booleans against outside oracles (chapter 28.10; piece G154): CGAL's Nef polyhedra compared
//! exactly; Manifold, manifold-rust's rational engine and Cherchi's exact booleans, whose results
//! leave them as decimals, by volume, area and two-sided Hausdorff distance.
//!
//! What it does: [`jobs`] draws a night's seeded pairs of closed solids with integer corners and a
//! boolean to apply: boxes on a coarse lattice, boxes that touch or nest, prisms and tetrahedra.
//! Each job is one JSON line in whole grid steps, so an exact oracle reads what the kernel read and
//! a floating-point one reads integers an `f64` holds exactly, with no conversion to round. The
//! harness in the oracle repository answers one JSON line per job. [`judge`] compares an answer
//! with `geom-mesh`'s boolean, and settles a difference by the exact reference
//! ([`crate::flat_reference`]): whether the kernel's volume is the reference's.
//!
//! Guarantees:
//!
//! - **Exact oracles are compared exactly**: a rational volume, equal or not.
//! - **The others can only refute**: volume and area are held to both sides' stated bounds, capped
//!   at ours plus the fit bound ([`Allowance`]), and the Hausdorff distance is measured by dense
//!   deterministic sampling, so it is a lower bound on the true distance.
//! - **Refusals are compared**: a job one side refuses and the other answers is a difference, of
//!   kind `refused`, never a pass.
//! - **Truth settles it**: an oracle is never trusted; [`Compared::Differ`]'s `settled` says what
//!   the reference found.
//!
//! Method: chapter 28.10 (`projects/engines/geometry/architecture/28-testing.md`). A solid's volume
//! is the divergence theorem over a fan of each loop; a point on a face or an edge is Ericson,
//! "Real-Time Collision Detection", 2005, section 5.1.5, closest point on a triangle.
//!
//! Cost: a job's boolean is the kernel's; sampling is the sample points times the triangles of the
//! other side, on solids of a few dozen triangles; the exact reference runs only on a difference.

use core::num::NonZeroU64;

use dashu_ratio::RBig;
use geom_format::operation::Operation;
use geom_format::payload::Reader;
use geom_mesh::boolean::boolean;
use geom_mesh::exact::build::{Corner, Polygons, Source, build};
use geom_mesh::exact::{ExactMesh, PropTable, Role, Vertex};
use geom_mesh::export::triangles;
use geom_num::grid::P3;
use geom_num::ladder::FIT;
use geom_num::plane::Plane;
use geom_op::cbor::Encoder;
use geom_op::{Exactness, NameId, NameSet, OpId, OpPath};
use geom_poly::winding::WindingTest;
use geom_run::{Budget, Exec, WorkMeter};
use serde::{Deserialize, Serialize};

use super::compare::{Allowance, exact, within};
use super::exact::{float, read_float};
pub use super::geomdl::Compared;
use super::triage::Signature;
use crate::fixture::Spend;
use crate::fixture::run::{Ran, Step};
use crate::flat_reference::{self, Point, Polygon, Q};
use crate::golden::Sha256;
use crate::rng::SplitMix64;
use crate::seed::Seed;

/// Manifold, out of process.
pub const MANIFOLD: &str = "manifold";
/// manifold-rust's rational engine, in process, a development dependency.
pub const MANIFOLD_RUST: &str = "manifold-rust";
/// CGAL's Nef polyhedra, out of process.
pub const CGAL: &str = "cgal";
/// Cherchi, Pellacini, Attene and Livesu's exact booleans, out of process.
pub const CHERCHI: &str = "cherchi";

/// Whether `oracle` is one of this module's.
#[must_use]
pub fn holds(oracle: &str) -> bool {
    [MANIFOLD, MANIFOLD_RUST, CGAL, CHERCHI].contains(&oracle)
}

/// Whether `oracle` answers in exact rationals.
fn is_exact(oracle: &str) -> bool {
    oracle == CGAL
}

/// The boolean of a job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    /// `a` or `b`.
    Union,
    /// `a` and `b`.
    Intersection,
    /// `a` and not `b`.
    Difference,
}

impl Op {
    /// The operation's name in a signature.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Op::Union => "mesh.union",
            Op::Intersection => "mesh.intersection",
            Op::Difference => "mesh.difference",
        }
    }

    fn test(self) -> WindingTest {
        match self {
            Op::Union => WindingTest::unite(),
            Op::Intersection => WindingTest::intersect(2),
            Op::Difference => WindingTest::subtract(0),
        }
    }
}

/// A closed solid: integer corners in grid steps, and triangles counter-clockwise from outside.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Solid {
    /// The corners.
    pub vertices: Vec<[i64; 3]>,
    /// The triangles, by corner.
    pub triangles: Vec<[u32; 3]>,
}

/// One job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    /// Its number.
    pub id: u64,
    /// Its input family.
    pub family: String,
    /// What to do.
    pub op: Op,
    /// The first solid.
    pub a: Solid,
    /// The second solid.
    pub b: Solid,
}

/// One answer. An exact oracle gives `volume` alone, as an integer or a fraction; a floating-point
/// one gives it as a double, with the rest.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    /// The job's number.
    pub id: u64,
    /// The result's volume, in cubic grid steps; `None` if the oracle refused the job.
    pub volume: Option<String>,
    /// The result's area, in square grid steps.
    #[serde(default)]
    pub area: Option<String>,
    /// How far the oracle says a vertex may be from where it belongs, in grid steps.
    #[serde(default)]
    pub tolerance: Option<String>,
    /// The result's triangles, each three points of three coordinates.
    #[serde(default)]
    pub triangles: Option<Vec<[[String; 3]; 3]>>,
}

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    NonZeroU64::new(n).map_or(0, |n| rng.below(n))
}

fn coordinate(rng: &mut SplitMix64, n: u64) -> i64 {
    i64::try_from(below(rng, n)).unwrap_or(0)
}

/// The box from `low` to `high`.
#[must_use]
pub fn cuboid(low: [i64; 3], high: [i64; 3]) -> Solid {
    let [a, b, c] = low;
    let [x, y, z] = high;
    Solid {
        vertices: vec![
            [a, b, c],
            [x, b, c],
            [x, y, c],
            [a, y, c],
            [a, b, z],
            [x, b, z],
            [x, y, z],
            [a, y, z],
        ],
        triangles: vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ],
    }
}

/// The prism over the convex outline `base`, counter-clockwise, from height `low` to `high`.
#[must_use]
pub fn prism(base: &[[i64; 2]], low: i64, high: i64) -> Solid {
    let n = u32::try_from(base.len()).unwrap_or(0);
    let mut vertices = Vec::new();
    for z in [low, high] {
        vertices.extend(base.iter().map(|&[x, y]| [x, y, z]));
    }
    let mut triangles = Vec::new();
    for i in 1..n.saturating_sub(1) {
        let next = i.saturating_add(1);
        triangles.push([0, next, i]);
        triangles.push([n, n.saturating_add(i), n.saturating_add(next)]);
    }
    for i in 0..n {
        let j = i.saturating_add(1).checked_rem(n).unwrap_or(0);
        let (top_i, top_j) = (n.saturating_add(i), n.saturating_add(j));
        triangles.push([i, j, top_j]);
        triangles.push([i, top_j, top_i]);
    }
    Solid {
        vertices,
        triangles,
    }
}

/// `b - a`.
fn minus(a: [i64; 3], b: [i64; 3]) -> [i64; 3] {
    let [x, y, z] = a;
    let [u, v, w] = b;
    [
        u.saturating_sub(x),
        v.saturating_sub(y),
        w.saturating_sub(z),
    ]
}

/// The determinant of three small rows.
fn determinant(p: [i64; 3], q: [i64; 3], r: [i64; 3]) -> i64 {
    let [a, b, c] = p;
    let [d, e, f] = q;
    let [g, h, i] = r;
    let minor =
        |x: i64, y: i64, z: i64, w: i64| x.saturating_mul(y).saturating_sub(z.saturating_mul(w));
    a.saturating_mul(minor(e, i, f, h))
        .saturating_sub(b.saturating_mul(minor(d, i, f, g)))
        .saturating_add(c.saturating_mul(minor(d, h, e, g)))
}

/// The tetrahedron on `corners`, if they span one.
#[must_use]
pub fn tetrahedron(corners: [[i64; 3]; 4]) -> Option<Solid> {
    let [a, mut b, mut c, d] = corners;
    let turn = determinant(minus(a, b), minus(a, c), minus(a, d));
    if turn == 0 {
        return None;
    }
    if turn < 0 {
        core::mem::swap(&mut b, &mut c);
    }
    Some(Solid {
        vertices: vec![a, b, c, d],
        triangles: vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [0, 3, 2]],
    })
}

/// Outlines of a coarse lattice: ties on every side.
const OUTLINES: [&[[i64; 2]]; 3] = [
    &[[0, 0], [8, 0], [4, 8]],
    &[[0, 0], [8, 0], [8, 4], [4, 8], [0, 4]],
    &[
        [2, 0],
        [6, 0],
        [8, 2],
        [8, 6],
        [6, 8],
        [2, 8],
        [0, 6],
        [0, 2],
    ],
];

/// A prism of `OUTLINES` at a lattice offset.
fn drawn_prism(rng: &mut SplitMix64) -> Solid {
    let chosen = usize::try_from(below(rng, 3)).unwrap_or(0);
    let outline = OUTLINES.get(chosen).copied().unwrap_or_default();
    let (x, y) = (
        coordinate(rng, 5).saturating_mul(2),
        coordinate(rng, 5).saturating_mul(2),
    );
    let moved: Vec<[i64; 2]> = outline
        .iter()
        .map(|&[u, v]| [u.saturating_add(x), v.saturating_add(y)])
        .collect();
    let low = coordinate(rng, 4).saturating_mul(2);
    prism(
        &moved,
        low,
        low.saturating_add(2)
            .saturating_add(coordinate(rng, 4).saturating_mul(2)),
    )
}

fn drawn_tetrahedron(rng: &mut SplitMix64) -> Solid {
    loop {
        let mut corner = || {
            [
                coordinate(rng, 17),
                coordinate(rng, 17),
                coordinate(rng, 17),
            ]
        };
        let corners = [corner(), corner(), corner(), corner()];
        if let Some(solid) = tetrahedron(corners) {
            return solid;
        }
    }
}

/// A box on a lattice of twos, from 0 to at most 16.
fn drawn_box(rng: &mut SplitMix64) -> Solid {
    let mut low = [0; 3];
    let mut high = [0; 3];
    for (l, h) in low.iter_mut().zip(&mut high) {
        *l = coordinate(rng, 7).saturating_mul(2);
        *h = (*l)
            .saturating_add(2)
            .saturating_add(coordinate(rng, 3).saturating_mul(2));
    }
    cuboid(low, high)
}

/// The two solids of family `family`, and its name.
fn drawn(rng: &mut SplitMix64, family: u64) -> (&'static str, Solid, Solid) {
    match family {
        0 => ("boxes", drawn_box(rng), drawn_box(rng)),
        1 => {
            let side = coordinate(rng, 4).saturating_add(1).saturating_mul(2);
            let far = side.saturating_add(4);
            (
                "flush",
                cuboid([0, 0, 0], [side, 8, 8]),
                cuboid([side, 0, 0], [far, 8, 8]),
            )
        }
        2 => {
            let inset = coordinate(rng, 3).saturating_add(1);
            (
                "nested",
                cuboid([0, 0, 0], [12, 12, 12]),
                cuboid([inset, inset, inset], [11, 11, 11]),
            )
        }
        3 => ("prisms", drawn_prism(rng), drawn_prism(rng)),
        _ => ("tetrahedra", drawn_tetrahedron(rng), drawn_box(rng)),
    }
}

/// The night's `count` jobs from `seed`.
#[must_use]
pub fn jobs(seed: Seed, count: u64) -> Vec<Job> {
    let mut rng = SplitMix64::new(seed);
    let mut out = Vec::new();
    for id in 0..count {
        let family = below(&mut rng, 5);
        let (name, a, b) = drawn(&mut rng, family);
        let op = match below(&mut rng, 3) {
            0 => Op::Union,
            1 => Op::Intersection,
            _ => Op::Difference,
        };
        out.push(Job {
            id,
            family: name.to_owned(),
            op,
            a,
            b,
        });
    }
    out
}

/// A job's solid as polygons of one run named by `name`.
fn polygons(solid: &Solid, name: u8) -> Result<Polygons, String> {
    let vertices = solid
        .vertices
        .iter()
        .map(|&[x, y, z]| {
            P3::new(x, y, z)
                .map(Corner::Grid)
                .map_err(|e| format!("a corner: {e:?}"))
        })
        .collect::<Result<Vec<Corner>, String>>()?;
    let faces = u32::try_from(solid.triangles.len()).map_err(|e| e.to_string())?;
    Ok(Polygons {
        vertices,
        corners: solid.triangles.iter().flatten().copied().collect(),
        loop_start: (0..=faces).map(|f| f.saturating_mul(3)).collect(),
        face_loops: (0..=faces).collect(),
        face_run: vec![0; solid.triangles.len()],
        runs: vec![Source {
            name: NameId::from_bytes([name; 16]),
            role: Role::Face,
            flags: 0,
        }],
        face_plane: Vec::new(),
        edge_plane: Vec::new(),
        corner_prop: Vec::new(),
        props: PropTable::default(),
        face_dev: Vec::new(),
        class: Exactness::Exact,
    })
}

/// The boolean of `job`, as the kernel makes it.
///
/// # Errors
///
/// The kernel's refusal, in words.
pub fn run(job: &Job) -> Result<ExactMesh, String> {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let exec = Exec::serial();
    let a = build(&polygons(&job.a, 1)?, &exec, &mut meter).map_err(|e| format!("{e:?}"))?;
    let b = build(&polygons(&job.b, 2)?, &exec, &mut meter).map_err(|e| format!("{e:?}"))?;
    let path = OpPath::new(OpId::from_bytes([0x72; 16]));
    let made = boolean(&[&a, &b], &[job.op.test()], &path, &exec, &mut meter)
        .map_err(|e| format!("{e:?}"))?;
    made.into_iter()
        .next()
        .map(|outcome| outcome.mesh)
        .ok_or_else(|| "no result".to_owned())
}

/// Where three planes meet, exactly, by Cramer's rule on n · x = -d.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "exact rationals, which cannot overflow"
)]
fn meet(planes: [&Plane; 3]) -> Result<[RBig; 3], String> {
    let rows = planes.map(|p| {
        let [x, y, z] = p.n();
        ([x, y, z].map(RBig::from), RBig::from(-p.d()))
    });
    let [(a, p), (b, q), (c, r)] = rows;
    let det = |u: &[RBig; 3], v: &[RBig; 3], w: &[RBig; 3]| {
        let [u0, u1, u2] = u;
        let [v0, v1, v2] = v;
        let [w0, w1, w2] = w;
        u0.clone() * (v1.clone() * w2.clone() - v2.clone() * w1.clone())
            - u1.clone() * (v0.clone() * w2.clone() - v2.clone() * w0.clone())
            + u2.clone() * (v0.clone() * w1.clone() - v1.clone() * w0.clone())
    };
    let weight = det(&a, &b, &c);
    if weight == RBig::ZERO {
        return Err("three planes that do not meet in a point".to_owned());
    }
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    let [c0, c1, c2] = c;
    let x = det(
        &[p.clone(), a1.clone(), a2.clone()],
        &[q.clone(), b1.clone(), b2.clone()],
        &[r.clone(), c1.clone(), c2.clone()],
    );
    let y = det(
        &[a0.clone(), p.clone(), a2],
        &[b0.clone(), q.clone(), b2],
        &[c0.clone(), r.clone(), c2],
    );
    let z = det(&[a0, a1, p], &[b0, b1, q], &[c0, c1, r]);
    Ok([x / weight.clone(), y / weight.clone(), z / weight])
}

/// A vertex of `mesh`, exactly.
fn corner(mesh: &ExactMesh, vertex: Vertex) -> Result<[RBig; 3], String> {
    match vertex {
        Vertex::Grid(p) => Ok([p.x.get(), p.y.get(), p.z.get()].map(RBig::from)),
        Vertex::Planes(rows) => {
            let plane = |row: u32| {
                usize::try_from(row)
                    .ok()
                    .and_then(|row| mesh.parts().planes.get(row))
                    .ok_or_else(|| "a plane past the pool".to_owned())
            };
            let [p, q, r] = rows;
            meet([plane(p)?, plane(q)?, plane(r)?])
        }
    }
}

/// Each face of `mesh` as its loops, each loop its corners' rows in the vertex table.
fn faces_of(mesh: &ExactMesh) -> Result<Vec<Vec<&[u32]>>, String> {
    let parts = mesh.parts();
    let wrong = || "a loop past the half-edges".to_owned();
    let mut faces = Vec::new();
    for pair in parts.face_loops.windows(2) {
        let [first, last] = pair else {
            continue;
        };
        let mut rings = Vec::new();
        for looped in *first..*last {
            let at = looped as usize;
            let start = parts.loop_start.get(at).ok_or_else(wrong)?;
            let end = parts
                .loop_start
                .get(at.saturating_add(1))
                .ok_or_else(wrong)?;
            let ring = parts.he_vert.get(*start as usize..*end as usize);
            rings.push(ring.ok_or_else(wrong)?);
        }
        faces.push(rings);
    }
    Ok(faces)
}

/// Every vertex of `mesh`, exactly.
fn corners_of(mesh: &ExactMesh) -> Result<Vec<[RBig; 3]>, String> {
    mesh.parts()
        .verts
        .iter()
        .map(|v| corner(mesh, *v))
        .collect()
}

fn row<'c>(corners: &'c [[RBig; 3]], at: Option<&u32>) -> Result<&'c [RBig; 3], String> {
    at.and_then(|v| corners.get(*v as usize))
        .ok_or_else(|| "a vertex past the table".to_owned())
}

/// The volume of closed `mesh`, exactly: six times the volume is the sum, over a fan of each loop,
/// of the determinants of its triangles.
///
/// # Errors
///
/// A mesh whose rows do not fit together, which only a defect can cause.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "exact rationals, which cannot overflow"
)]
pub fn volume(mesh: &ExactMesh) -> Result<RBig, String> {
    let corners = corners_of(mesh)?;
    let mut six = RBig::ZERO;
    for ring in faces_of(mesh)?.into_iter().flatten() {
        let [a0, a1, a2] = row(&corners, ring.first())?;
        for fan in ring.get(1..).unwrap_or_default().windows(2) {
            let [b0, b1, b2] = row(&corners, fan.first())?;
            let [c0, c1, c2] = row(&corners, fan.get(1))?;
            six += a0.clone() * (b1.clone() * c2.clone() - b2.clone() * c1.clone())
                - a1.clone() * (b0.clone() * c2.clone() - b2.clone() * c0.clone())
                + a2.clone() * (b0.clone() * c1.clone() - b1.clone() * c0.clone());
        }
    }
    Ok(six / RBig::from(6))
}

/// The area of `mesh` and the length of its loops: a face's area is the size of its vector area,
/// summed exactly over its loops, so a hole takes itself off and no rounding of a corner shows.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "exact rationals, and doubles, which cannot overflow here"
)]
fn sizes(mesh: &ExactMesh) -> Result<(f64, f64), String> {
    let corners = corners_of(mesh)?;
    let (mut area, mut edges) = (0.0, 0.0);
    for rings in faces_of(mesh)? {
        let mut twice = [RBig::ZERO, RBig::ZERO, RBig::ZERO];
        for ring in rings {
            let [a0, a1, a2] = row(&corners, ring.first())?;
            for fan in ring.get(1..).unwrap_or_default().windows(2) {
                let [b0, b1, b2] = row(&corners, fan.first())?;
                let [c0, c1, c2] = row(&corners, fan.get(1))?;
                let (u0, u1, u2) = (b0 - a0, b1 - a1, b2 - a2);
                let (v0, v1, v2) = (c0 - a0, c1 - a1, c2 - a2);
                let [x, y, z] = &mut twice;
                *x += u1.clone() * v2.clone() - u2.clone() * v1.clone();
                *y += u2 * v0.clone() - u0.clone() * v2;
                *z += u0 * v1 - u1 * v0;
            }
            for (k, at) in ring.iter().enumerate() {
                let next = ring.get(k.saturating_add(1)).or(ring.first());
                let [p0, p1, p2] = row(&corners, Some(at))?;
                let [q0, q1, q2] = row(&corners, next)?;
                let gap = [q0 - p0, q1 - p1, q2 - p2].map(|g| g.to_f64().value());
                edges += dot(gap, gap).sqrt();
            }
        }
        let vector = twice.map(|x| x.to_f64().value());
        area += dot(vector, vector).sqrt() / 2.0;
    }
    Ok((area, edges))
}

/// One triangle.
type Triangle = [[f64; 3]; 3];

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let [x, y, z] = a;
    let [u, v, w] = b;
    [x - u, y - v, z - w]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    let [x, y, z] = a;
    let [u, v, w] = b;
    x * u + y * v + z * w
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let [x, y, z] = a;
    let [u, v, w] = b;
    [y * w - z * v, z * u - x * w, x * v - y * u]
}

fn along(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    let [x, y, z] = a;
    let [u, v, w] = b;
    [x + t * u, y + t * v, z + t * w]
}

/// How far `p` is from triangle `[a, b, c]`: Ericson's regions of the triangle, in order.
fn distance(p: [f64; 3], triangle: &Triangle) -> f64 {
    let &[a, b, c] = triangle;
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    let (bp, cp) = (sub(p, b), sub(p, c));
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    let (vc, vb, va) = (d1 * d4 - d3 * d2, d5 * d2 - d1 * d6, d3 * d6 - d5 * d4);
    let closest = if d1 <= 0.0 && d2 <= 0.0 {
        a
    } else if d3 >= 0.0 && d4 <= d3 {
        b
    } else if d6 >= 0.0 && d5 <= d6 {
        c
    } else if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        along(a, ab, d1 / (d1 - d3))
    } else if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        along(a, ac, d2 / (d2 - d6))
    } else if va <= 0.0 && d4 >= d3 && d5 >= d6 {
        along(b, sub(c, b), (d4 - d3) / ((d4 - d3) + (d5 - d6)))
    } else {
        let scale = 1.0 / (va + vb + vc);
        along(along(a, ab, vb * scale), ac, vc * scale)
    };
    let gap = sub(p, closest);
    dot(gap, gap).sqrt()
}

/// Samples along each side of a triangle.
const DENSITY: u32 = 8;

/// The gap, in grid steps, sampling cannot see: on a sliver of a triangle a point's distance from
/// the triangle it came from rounds to about this, and it is far under anything that matters.
const SAMPLED: f64 = 1e-6;

/// The largest distance from a sample of `from` to the nearest of `to`.
fn one_way(from: &[Triangle], to: &[Triangle]) -> f64 {
    let mut far: f64 = 0.0;
    let density = f64::from(DENSITY);
    for &[a, b, c] in from {
        let (u, v) = (sub(b, a), sub(c, a));
        for i in 0..=DENSITY {
            for j in 0..=DENSITY.saturating_sub(i) {
                let p = along(
                    along(a, u, f64::from(i) / density),
                    v,
                    f64::from(j) / density,
                );
                let near = to
                    .iter()
                    .map(|t| distance(p, t))
                    .fold(f64::INFINITY, f64::min);
                far = far.max(near);
            }
        }
    }
    far
}

/// The two-sided Hausdorff distance between two triangle sets, by sampling, so it can only refute.
/// Infinite if exactly one set is empty.
#[must_use]
pub fn hausdorff(a: &[Triangle], b: &[Triangle]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    one_way(a, b).max(one_way(b, a))
}

/// What the kernel made of a job, as a comparison reads it: each corner the double nearest its
/// exact place, not the grid point it would be written at.
struct Made {
    volume: RBig,
    area: f64,
    edges: f64,
    triangles: Vec<Triangle>,
    /// Whether a corner was off the grid: triangles are drawn between rounded corners, so one may
    /// stray from its face by up to a step.
    rounded: bool,
}

fn made(mesh: &ExactMesh) -> Result<Made, String> {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let drawn = triangles(mesh, &mut meter).map_err(|e| format!("{e:?}"))?;
    let vertex = |v: u32| {
        let at = mesh
            .parts()
            .verts
            .get(v as usize)
            .ok_or_else(|| "a corner past the table".to_owned())?;
        Ok::<[f64; 3], String>(corner(mesh, *at)?.map(|x| x.to_f64().value()))
    };
    let mut all = Vec::with_capacity(drawn.corners.len());
    for &[a, b, c] in &drawn.corners {
        all.push([vertex(a)?, vertex(b)?, vertex(c)?]);
    }
    let (area, edges) = sizes(mesh)?;
    Ok(Made {
        volume: volume(mesh)?,
        area,
        edges,
        triangles: all,
        rounded: drawn.rounded,
    })
}

/// A job's solid for the reference.
fn reference_solid(solid: &Solid) -> Result<flat_reference::Solid, String> {
    let error = |e| format!("the reference: {e:?}");
    let point = |v: u32| -> Result<Point, String> {
        let [x, y, z] = solid
            .vertices
            .get(v as usize)
            .copied()
            .ok_or_else(|| "a corner past the table".to_owned())?;
        Ok(flat_reference::point(x, y, z))
    };
    let mut faces = Vec::with_capacity(solid.triangles.len());
    for &[a, b, c] in &solid.triangles {
        faces.push(Polygon::new(vec![point(a)?, point(b)?, point(c)?]).map_err(error)?);
    }
    flat_reference::Solid::new(faces).map_err(error)
}

/// The exact reference's volume for `job`, written from the definitions alone.
///
/// # Errors
///
/// A job the reference cannot read.
pub fn reference(job: &Job) -> Result<Q, String> {
    let op = match job.op {
        Op::Union => flat_reference::Op::Union,
        Op::Intersection => flat_reference::Op::Intersection,
        Op::Difference => flat_reference::Op::Difference,
    };
    let (a, b) = (reference_solid(&job.a)?, reference_solid(&job.b)?);
    flat_reference::boolean(&a, &b, op)
        .map(|region| region.volume())
        .map_err(|e| format!("the reference: {e:?}"))
}

/// What the reference finds of the kernel's `volume` for `job`.
fn settle(job: &Job, volume: &RBig) -> Result<String, String> {
    let truth = reference(job)?;
    Ok(if truth == *volume {
        "theirs: the kernel's volume is the exact reference's"
    } else {
        "ours: the kernel's volume is not the exact reference's"
    }
    .to_owned())
}

/// The difference `job` raised at `oracle`, of `kind`, with `settled`.
fn difference(oracle: &str, job: &Job, kind: &str, settled: String) -> Compared {
    Compared::Differ {
        signature: Signature {
            operation: job.op.name().to_owned(),
            oracle: oracle.to_owned(),
            kind: kind.to_owned(),
            family: job.family.clone(),
        },
        settled,
    }
}

/// `x` from its text.
fn double(text: Option<&String>, what: &str, id: u64) -> Result<f64, String> {
    text.and_then(|t| read_float(t))
        .ok_or_else(|| format!("job {id}: no {what}"))
}

/// An answer's triangles.
fn answered(answer: &Answer) -> Result<Vec<Triangle>, String> {
    let wrong = || format!("job {}: bad triangles", answer.id);
    let mut out = Vec::new();
    for triangle in answer.triangles.as_ref().ok_or_else(wrong)? {
        let mut read = [[0.0; 3]; 3];
        for (row, texts) in read.iter_mut().zip(triangle) {
            for (x, text) in row.iter_mut().zip(texts) {
                *x = read_float(text).ok_or_else(wrong)?;
            }
        }
        out.push(read);
    }
    Ok(out)
}

/// `made` against a floating-point oracle's `answer`.
fn floating(oracle: &str, job: &Job, ours: &Made, answer: &Answer) -> Result<Compared, String> {
    let theirs = double(answer.volume.as_ref(), "volume", job.id)?;
    let area = double(answer.area.as_ref(), "area", job.id)?;
    // Manifold states -1 for an empty mesh: nothing to be off by.
    let stated = double(answer.tolerance.as_ref(), "tolerance", job.id)?.max(0.0);
    let shape = answered(answer)?;
    let (our_area, edges) = (ours.area, ours.edges);
    let fit = f64::from(u32::try_from(FIT.0).unwrap_or(u32::MAX));
    // Ours are exact, and doubles only here: what that rounds is `rounding`.
    let allowance = |unit: f64, moved: f64, rounding: f64| Allowance {
        ours: moved * unit,
        theirs: stated * unit,
        rounding,
        fit: fit * unit,
    };
    let our_volume = ours.volume.to_f64().value();
    let by_volume = allowance(our_area.max(area), 0.0, our_volume.abs() * f64::EPSILON);
    let by_area = allowance(edges, 0.0, our_area * f64::EPSILON * 16.0);
    let moved = if ours.rounded { 1.0 } else { 0.0 };
    let by_gap = allowance(1.0, moved, SAMPLED);
    let kind = if !within(our_volume, theirs, &by_volume) {
        "volume"
    } else if !within(our_area, area, &by_area) {
        "area"
    } else if hausdorff(&ours.triangles, &shape) > by_gap.total() {
        "hausdorff"
    } else {
        return Ok(Compared::Agree);
    };
    Ok(difference(oracle, job, kind, settle(job, &ours.volume)?))
}

/// Judges one answer of `oracle`.
///
/// # Errors
///
/// A job or answer that cannot be read: the harness is broken, not the kernel.
pub fn judge(oracle: &str, job: &Job, answer: &Answer) -> Result<Compared, String> {
    if job.id != answer.id {
        return Err(format!("answer {} for job {}", answer.id, job.id));
    }
    let ours = run(job);
    let (mesh, theirs) = match (&ours, &answer.volume) {
        (Err(_), None) => return Ok(Compared::Agree),
        (Err(why), Some(_)) => {
            return Ok(difference(oracle, job, "refused", why.clone()));
        }
        (Ok(_), None) => {
            return Ok(difference(
                oracle,
                job,
                "refused",
                "the oracle refused the job".to_owned(),
            ));
        }
        (Ok(mesh), Some(theirs)) => (mesh, theirs),
    };
    let made = made(mesh)?;
    if !is_exact(oracle) {
        return floating(oracle, job, &made, answer);
    }
    let theirs: RBig = theirs
        .parse()
        .map_err(|e| format!("job {}: the volume {theirs:?}: {e:?}", job.id))?;
    if exact(&made.volume, &theirs) {
        return Ok(Compared::Agree);
    }
    Ok(difference(
        oracle,
        job,
        "volume",
        settle(job, &made.volume)?,
    ))
}

/// `job` as the one operation of a log: its operation's name, and its JSON as the parameters, so a
/// triaged case replays through the kernel (`fixture::kernel`).
///
/// # Errors
///
/// A job with no encoding, which only a defect can cause.
pub fn operation(job: &Job) -> Result<Operation, String> {
    let json = serde_json::to_string(job).map_err(|e| e.to_string())?;
    let mut e = Encoder::new();
    e.text(&json).map_err(|e| e.to_string())?;
    Ok(Operation {
        op: OpPath::new(OpId::from_bytes([0x72; 16])),
        name: job.op.name().to_owned(),
        version: 1,
        inputs: Vec::new(),
        params: e.finish().map_err(|e| e.to_string())?.into_bytes(),
        budget: Budget::UNLIMITED,
    })
}

/// Runs an operation [`operation`] wrote; `None` for any other operation.
#[must_use]
pub fn replay(operation: &Operation) -> Option<Ran> {
    if ![Op::Union, Op::Intersection, Op::Difference]
        .iter()
        .any(|op| op.name() == operation.name)
    {
        return None;
    }
    Some(replayed(operation).unwrap_or_else(Ran::Refused))
}

fn replayed(operation: &Operation) -> Result<Ran, String> {
    if operation.version != 1 {
        return Err(format!("no version {}", operation.version));
    }
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let mut reader = Reader::new(&operation.params);
    let text = reader
        .text(&mut meter, "a job")
        .map_err(|e| e.to_string())?;
    reader.end().map_err(|e| e.to_string())?;
    let job: Job = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let mesh = run(&job)?;
    let mut sha = Sha256::new();
    sha.write(volume(&mesh)?.to_string().as_bytes());
    Ok(Ran::Done(Step {
        bodies: vec![sha.finish()],
        names: NameSet::default(),
        history: None,
        exactness: Exactness::Exact,
        diagnostics: Vec::new(),
        work: Spend::default(),
    }))
}

/// An answer of the kernel's own: what a correct floating-point oracle gives for `job`. For tests
/// of the comparison, and for a harness's first run.
///
/// # Errors
///
/// A job the kernel refuses.
pub fn own_answer(job: &Job) -> Result<Answer, String> {
    let ours = made(&run(job)?)?;
    let text = |x: f64| float(x).unwrap_or_default();
    Ok(Answer {
        id: job.id,
        volume: Some(text(ours.volume.to_f64().value())),
        area: Some(text(ours.area)),
        tolerance: Some(text(0.0)),
        triangles: Some(
            ours.triangles
                .iter()
                .map(|t| t.map(|p| p.map(text)))
                .collect(),
        ),
    })
}

/// The kernel's exact volume for `job` as an exact oracle writes it.
///
/// # Errors
///
/// A job the kernel refuses.
pub fn own_volume(job: &Job) -> Result<String, String> {
    Ok(volume(&run(job)?)?.to_string())
}
