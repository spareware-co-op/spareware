//! Mesh booleans against outside oracles (chapter 28.10; piece G154): CGAL's Nef polyhedra compared
//! exactly; Manifold, manifold-rust's rational engine and Cherchi's exact booleans, whose results
//! leave them as decimals, by volume, area and two-sided Hausdorff distance.
//!
//! What it does: [`jobs`] draws a night's seeded pairs of closed solids with integer corners and a
//! boolean to apply: boxes on a coarse lattice, boxes that touch or nest, prisms and tetrahedra.
//! Each job is one JSON line in whole grid steps, so an exact oracle reads what the kernel read and
//! a floating-point one reads integers an `f64` holds exactly, with no conversion to round. Those
//! are the case set `seeded`; the set `manifold-models` ([`model_jobs`]) is Manifold's stored
//! models of the polyhedral corpus's smaller tier, each with a moved copy of itself, under each
//! boolean ([`CASE_SETS`] names every set, and [`case_jobs`] writes one). The harness in the
//! oracle repository answers one JSON line per job. [`judge`] compares an answer
//! with `geom-mesh`'s boolean, and settles a difference by the exact reference
//! ([`crate::flat_reference`]): whether the kernel's volume is the reference's. A model a case set
//! records as refused ([`REFUSED_MODELS`]) is not sent to an oracle, and the kernel is held to its
//! refusal ([`held_refusal`]). A job on a model that crosses itself ([`crosses`]), which the
//! oracle's documentation says it does not take ([`stated_input`]), is judged by the exact
//! reference alone ([`judge_by_reference`]; decision 32.59).
//!
//! Guarantees:
//!
//! - **Exact oracles are compared exactly**: a rational volume, equal or not.
//! - **The others can only refute**: volume and area are held to both sides' stated bounds, capped
//!   at ours plus the fit bound ([`Allowance`]), and the Hausdorff distance is measured by dense
//!   deterministic sampling, so it is a lower bound on the true distance. Each sample's distance to
//!   a triangle is itself a certified lower bound, never more than 10^-9 steps (and 2^-40 of the
//!   distance) below the truth: from the triangle's own corners in intervals, and in exact
//!   rationals where those cannot settle it.
//! - **Outside the oracle's input, the reference alone**: a job whose solid crosses itself, by the
//!   kernel's own exact winding numbers, is outside what Manifold's and CGAL's documentation say
//!   they take, so the oracle's answer is not read: the kernel's volume must be the exact
//!   reference's, or the job fails. The job is never a pass on the oracle's word.
//! - **A recorded refusal is held exactly**: a model the case set records as refused fails its
//!   jobs if the kernel accepts it or refuses it in another way.
//! - **The kernel's side is its exact result**: its volume and area are computed exactly from its
//!   corners, and the distance is measured from and to its exact faces, each corner the double
//!   nearest its place, never from the export's triangles, whose corners are rounded to the grid.
//!   So the oracle is held to the kernel's exact answer, and no rounding of ours is allowed for.
//! - **Refusals are compared**: a job one side refuses and the other answers is a difference, of
//!   kind `refused`, never a pass.
//! - **A result the kernel cannot measure is a difference**: a result whose corners cannot be
//!   placed, which only a defect can cause, is a difference of kind `unmeasured`, never a pass and
//!   never the end of the night.
//! - **Truth settles it**: an oracle is never trusted; [`Compared::Differ`]'s `settled` says what
//!   the reference found.
//!
//! Method: chapter 28.10 (`projects/engines/geometry/architecture/28-testing.md`). A solid's volume
//! is the divergence theorem over a fan of each loop; a point's distance to a triangle is its
//! height over the triangle's plane where its foot is inside (three turns about the normal), or
//! else its distance to the nearest side, Ericson, "Real-Time Collision Detection", 2005, section
//! 5.1.2 on a segment, enclosed by interval arithmetic (Moore, Kearfott and Cloud, "Introduction
//! to Interval Analysis", 2009) and decided in exact rationals where an enclosure cannot; whether a
//! point of a face's plane is inside its loops is the crossing test
//! (W. Randolph Franklin's PNPOLY, as in Haines, "Point in Polygon Strategies", Graphics Gems IV,
//! 1994). An exact face is sampled along its sides and on a lattice over its box in the plan its
//! normal leans on least, lifted to its plane where the lattice is inside it.
//!
//! Cost: a job's boolean is the kernel's; sampling is the sample points times the triangles or
//! faces of the other side, on seeded solids of a few dozen and stored models of up to
//! [`MODEL_FACES`] each, a triangle whose box is certainly farther than the nearest so far passed
//! by; the exact reference runs on a difference, on each solid once per [`Judge`] to read whether
//! it crosses itself, and on each job of a solid that does.

use core::num::NonZeroU64;
use std::collections::BTreeMap;

use dashu_ratio::RBig;
use geom_format::operation::Operation;
use geom_format::payload::Reader;
use geom_mesh::boolean::boolean;
use geom_mesh::exact::build::{Corner, Polygons, Source, build};
use geom_mesh::exact::{ExactMesh, MeshError, MeshFault, PropTable, Role, Vertex};
use geom_mesh::export::triangles;
use geom_num::grid::P3;
use geom_num::interval::Interval;
use geom_num::ladder::FIT;
use geom_num::plane::Plane;
use geom_num::{NumError, Sign};
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
use crate::obj::{self, Grid};
use crate::polyhedral::MODELS;
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
#[allow(
    clippy::many_single_char_names,
    reason = "a, b and c are the low corner's coordinates and x, y and z the high one's"
)]
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
#[allow(
    clippy::many_single_char_names,
    reason = "two points and their coordinates, as in the formula"
)]
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
#[allow(
    clippy::many_single_char_names,
    reason = "three rows and their entries, as in the rule of Sarrus"
)]
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

/// The case set of seeded solids: [`jobs`], from a seed and a count.
pub const SEEDED: &str = "seeded";
/// The case set of Manifold's stored models: [`model_jobs`], the same every run.
pub const MANIFOLD_MODELS: &str = "manifold-models";
/// Every case set `oracle jobs` writes for a mesh oracle, by name: `kernel-oracles.yml`'s job
/// `meshes` runs each of them, and a set it names that is not here stops the run.
pub const CASE_SETS: [&str; 2] = [SEEDED, MANIFOLD_MODELS];

/// The models [`model_jobs`] takes: those of at most this many faces, the tier of the polyhedral
/// corpus that is held to the exact reference. The seven larger ones are `kernel.yml`'s job
/// `large-models`: the kernel takes minutes on each, and the reference, which settles a
/// difference, over an hour.
pub const MODEL_FACES: usize = 1_500;

/// A model on the grid moved by `by`, as a job's solid.
fn model_solid(grid: &Grid, by: [i64; 3]) -> Result<Solid, String> {
    let vertices = grid
        .points
        .iter()
        .map(|p| {
            let mut moved = [0; 3];
            for ((m, x), d) in moved.iter_mut().zip(p).zip(by) {
                *m = x.checked_add(d).ok_or("a coordinate past i64")?;
            }
            Ok(moved)
        })
        .collect::<Result<Vec<[i64; 3]>, String>>()?;
    let triangles = grid
        .faces
        .iter()
        .map(|face| match face.as_slice() {
            &[a, b, c] => Ok([a, b, c]),
            _ => Err(format!("a face of {} corners, not a triangle", face.len())),
        })
        .collect::<Result<Vec<[u32; 3]>, String>>()?;
    Ok(Solid {
        vertices,
        triangles,
    })
}

/// Where a model's copy goes: a third of its extent and a step along each axis, as the polyhedral
/// corpus's tests move it, so that the copy overlaps the model on every axis.
fn copy_offset(grid: &Grid) -> [i64; 3] {
    let mut by = [0; 3];
    for (k, d) in by.iter_mut().enumerate() {
        let along = grid.points.iter().filter_map(|p| p.get(k).copied());
        let (low, high) = (along.clone().min(), along.max());
        *d = high
            .zip(low)
            .map_or(0, |(h, l)| h.saturating_sub(l) / 3)
            .saturating_add(1);
    }
    by
}

/// Manifold's stored models (the corpus `manifold-models`, [`crate::polyhedral::MODELS`]) of at
/// most [`MODEL_FACES`] faces, each with its copy moved by [`copy_offset`], under each boolean in
/// turn: union, intersection and difference. A job's family is the set's name and the model's.
///
/// # Errors
///
/// A model that cannot be read onto the grid or is not made of triangles, which only a change to
/// the stored corpus can cause.
pub fn model_jobs() -> Result<Vec<Job>, String> {
    let mut out = Vec::new();
    for (name, text) in MODELS {
        let grid = obj::read(text)
            .map_err(|e| format!("{name}: {e}"))?
            .to_grid()
            .ok_or_else(|| format!("{name}: no scale puts it on the grid"))?;
        if grid.faces.len() > MODEL_FACES {
            continue;
        }
        let a = model_solid(&grid, [0; 3]).map_err(|e| format!("{name}: {e}"))?;
        let b = model_solid(&grid, copy_offset(&grid)).map_err(|e| format!("{name}: {e}"))?;
        for op in [Op::Union, Op::Intersection, Op::Difference] {
            out.push(Job {
                id: u64::try_from(out.len()).map_err(|e| e.to_string())?,
                family: format!("{MANIFOLD_MODELS}/{name}"),
                op,
                a: a.clone(),
                b: b.clone(),
            });
        }
    }
    Ok(out)
}

/// The jobs of the case set `cases`: [`SEEDED`]'s from `seeded`, its seed and count, which it
/// needs; a stored set's from nothing else, and it refuses a seed or a count.
///
/// # Errors
///
/// A set not in [`CASE_SETS`], the seeded set without its seed and count, a stored set given one,
/// or a stored set that cannot be read.
pub fn case_jobs(cases: &str, seeded: Option<(Seed, u64)>) -> Result<Vec<Job>, String> {
    match (cases, seeded) {
        (SEEDED, Some((seed, count))) => Ok(jobs(seed, count)),
        (SEEDED, None) => Err(format!("the case set {SEEDED} needs --seed and --count")),
        (MANIFOLD_MODELS, None) => model_jobs(),
        (MANIFOLD_MODELS, Some(_)) => Err(format!(
            "the case set {MANIFOLD_MODELS} is stored: it takes no --seed or --count"
        )),
        _ => Err(format!(
            "no case set {cases}; the sets are {}",
            CASE_SETS.join(", ")
        )),
    }
}

/// The models of [`MANIFOLD_MODELS`] that are not valid exact meshes on the grid, each with the
/// kernel's refusal of it, exactly (its kind and the place it names, as `geom-mesh`'s builder
/// gives it; the polyhedral corpus test's `REFUSED` holds the same four). Each has repeated
/// corners, faces of no area once on the grid, or edges of more than two faces: 19.2 has no such
/// face or edge in an exact mesh, and the route for such a model is import and heal (19.7), tested
/// where healing lives. The kernel is right to refuse them, so their jobs are not sent to an
/// oracle ([`asked`]): `oracle compare` holds the kernel to each refusal instead
/// ([`recorded_refusals`], [`held_refusal`]), and a job of them fails if the kernel accepts the
/// model or refuses it otherwise.
pub const REFUSED_MODELS: [(&str, MeshError); 4] = [
    (
        "dedupe_stale_duplicate",
        MeshError::Fault(MeshFault::DegenerateFace { face: 0 }),
    ),
    (
        "degenerate_split_quad_first",
        MeshError::Fault(MeshFault::DegenerateFace { face: 51 }),
    ),
    (
        "degenerate_swap_cycle_first",
        MeshError::Fault(MeshFault::UnevenEdge { half_edge: 227 }),
    ),
    (
        "openscad-nonmanifold-crash",
        MeshError::Fault(MeshFault::DegenerateFace { face: 1359 }),
    ),
];

/// The refusal [`REFUSED_MODELS`] records for `job`'s model, if any.
#[must_use]
pub fn recorded_refusal(job: &Job) -> Option<MeshError> {
    REFUSED_MODELS
        .iter()
        .find(|(name, _)| job.family == format!("{MANIFOLD_MODELS}/{name}"))
        .map(|(_, refusal)| *refusal)
}

/// Whether `job` is sent to the oracle: every job but those of a model recorded as refused.
#[must_use]
pub fn asked(job: &Job) -> bool {
    recorded_refusal(job).is_none()
}

/// Whether `job` is one of the case set `cases`'s: a stored set's jobs are of its families, and
/// the seeded set's of none of them.
#[must_use]
pub fn of_case_set(cases: &str, job: &Job) -> bool {
    let stored = job.family.starts_with(&format!("{MANIFOLD_MODELS}/"));
    match cases {
        SEEDED => !stored,
        MANIFOLD_MODELS => stored,
        _ => false,
    }
}

/// The jobs of the case set `cases` that are not sent to an oracle, each with the refusal it
/// records: none for the seeded set.
///
/// # Errors
///
/// A set not in [`CASE_SETS`], a stored set that cannot be read, or a model recorded as refused
/// that has no job in its set, so a record cannot fall out of use unseen.
pub fn recorded_refusals(cases: &str) -> Result<Vec<(Job, MeshError)>, String> {
    match cases {
        SEEDED => Ok(Vec::new()),
        MANIFOLD_MODELS => {
            let jobs = model_jobs()?;
            for (name, _) in REFUSED_MODELS {
                let family = format!("{MANIFOLD_MODELS}/{name}");
                if !jobs.iter().any(|j| j.family == family) {
                    return Err(format!("{name} is recorded as refused but has no job"));
                }
            }
            Ok(jobs
                .into_iter()
                .filter_map(|j| recorded_refusal(&j).map(|r| (j, r)))
                .collect())
        }
        _ => Err(format!(
            "no case set {cases}; the sets are {}",
            CASE_SETS.join(", ")
        )),
    }
}

/// The kernel's refusal of `job`'s solids, as its builder types it: the first solid's that it
/// refuses, or `None` if it builds both.
///
/// # Errors
///
/// A solid whose corners are past the grid, so it cannot be given to the builder.
pub fn refusal(job: &Job) -> Result<Option<MeshError>, String> {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let exec = Exec::serial();
    for (solid, name) in [(&job.a, 1), (&job.b, 2)] {
        if let Err(e) = build(&polygons(solid, name)?, &exec, &mut meter) {
            return Ok(Some(e));
        }
    }
    Ok(None)
}

/// Whether the kernel refuses `job` with exactly `expected`.
///
/// # Errors
///
/// What the kernel did instead, in words: accepted the model, or refused it otherwise.
pub fn held_refusal(job: &Job, expected: MeshError) -> Result<(), String> {
    match refusal(job)? {
        Some(found) if found == expected => Ok(()),
        Some(found) => Err(format!(
            "job {}: the kernel refuses {} with {found:?}, not the recorded {expected:?}",
            job.id, job.family
        )),
        None => Err(format!(
            "job {}: the kernel accepts {}, recorded as refused with {expected:?}",
            job.id, job.family
        )),
    }
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
#[allow(
    clippy::many_single_char_names,
    reason = "three planes' rows a, b and c and offsets p, q and r, as in Cramer's rule"
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

/// The larger of two doubles neither of which is NaN: `f64::max` is banned, as its sign of zero is
/// not fixed.
fn larger(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// The smaller of two doubles neither of which is NaN.
fn smaller(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

#[allow(
    clippy::many_single_char_names,
    reason = "two points and their coordinates, as in the formula"
)]
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let [x, y, z] = a;
    let [u, v, w] = b;
    [x - u, y - v, z - w]
}

#[allow(
    clippy::many_single_char_names,
    reason = "two vectors and their coordinates, as in the formula"
)]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    let [x, y, z] = a;
    let [u, v, w] = b;
    x * u + y * v + z * w
}

#[allow(
    clippy::many_single_char_names,
    reason = "a point, a direction and their coordinates, as in the formula"
)]
fn along(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    let [x, y, z] = a;
    let [u, v, w] = b;
    [x + t * u, y + t * v, z + t * w]
}

/// How far below the true distance a certified one may be, in grid steps: a thousandth of what
/// sampling cannot see ([`SAMPLED`]), and with it [`WIDE`] of the distance itself.
const CERTIFIED: f64 = 1e-9;

/// The part of a distance its enclosure may be wide by besides [`CERTIFIED`]: 2^-40.
const WIDE: f64 = 1.0 / 1_099_511_627_776.0;

/// A point or a vector of enclosures.
type Enclosed = [Interval; 3];

fn enclose(p: [f64; 3]) -> Result<Enclosed, NumError> {
    let [x, y, z] = p;
    Ok([
        Interval::point(x)?,
        Interval::point(y)?,
        Interval::point(z)?,
    ])
}

#[allow(
    clippy::many_single_char_names,
    reason = "two vectors and their coordinates, as in the formula"
)]
fn less(a: Enclosed, b: Enclosed) -> Result<Enclosed, NumError> {
    let [x, y, z] = a;
    let [u, v, w] = b;
    Ok([x.sub(u)?, y.sub(v)?, z.sub(w)?])
}

#[allow(
    clippy::many_single_char_names,
    reason = "two vectors and their coordinates, as in the formula"
)]
fn inner(a: Enclosed, b: Enclosed) -> Result<Interval, NumError> {
    let [x, y, z] = a;
    let [u, v, w] = b;
    x.mul(u)?.add(y.mul(v)?)?.add(z.mul(w)?)
}

#[allow(
    clippy::many_single_char_names,
    reason = "two vectors and their coordinates, as in the formula"
)]
fn outer(a: Enclosed, b: Enclosed) -> Result<Enclosed, NumError> {
    let [x, y, z] = a;
    let [u, v, w] = b;
    Ok([
        y.mul(w)?.sub(z.mul(v)?)?,
        z.mul(u)?.sub(x.mul(w)?)?,
        x.mul(v)?.sub(y.mul(u)?)?,
    ])
}

/// The smallest interval holding both `a` and `b`.
fn hull(a: Interval, b: Interval) -> Result<Interval, NumError> {
    Interval::new(smaller(a.lo(), b.lo()), larger(a.hi(), b.hi()))
}

/// An enclosure of the nearer of two distances enclosed by `a` and `b`.
fn nearer(a: Interval, b: Interval) -> Result<Interval, NumError> {
    Interval::new(smaller(a.lo(), b.lo()), smaller(a.hi(), b.hi()))
}

/// Whether every value of `x` is at least zero, or at most zero.
fn at_least_zero(x: Interval) -> bool {
    matches!(x.certain_sign(), Some(Sign::Positive | Sign::Zero))
}

fn at_most_zero(x: Interval) -> bool {
    matches!(x.certain_sign(), Some(Sign::Negative | Sign::Zero))
}

/// An enclosure of how far the point `v` from a segment's start is from the segment, whose end is
/// `e` from its start: from the start, the end or the line between, as the foot of `v` on the line
/// falls. Where the enclosures cannot tell where the foot falls, the hull of each place it may,
/// since the true distance is the one of the place it does. `None` for a segment that may have no
/// length, which [`exact_reach`] decides.
fn segment_enclosed(v: Enclosed, e: Enclosed) -> Result<Option<Interval>, NumError> {
    let length = Interval::norm(&e)?;
    if length.lo() <= 0.0 {
        return Ok(None);
    }
    let foot = inner(v, e)?;
    let past = foot.sub(inner(e, e)?)?;
    let mut out: Option<Interval> = None;
    let mut take = |d: Interval| -> Result<(), NumError> {
        out = Some(match out {
            Some(o) => hull(o, d)?,
            None => d,
        });
        Ok(())
    };
    if !matches!(foot.certain_sign(), Some(Sign::Positive)) {
        take(Interval::norm(&v)?)?;
    }
    if !matches!(past.certain_sign(), Some(Sign::Negative)) {
        take(Interval::norm(&less(v, e)?)?)?;
    }
    if !at_most_zero(foot) && !at_least_zero(past) {
        take(Interval::norm(&outer(v, e)?)?.div(length)?)?;
    }
    Ok(out)
}

/// An enclosure of how far `p` is from `triangle`, every quantity measured from the triangle's own
/// corners in intervals ([`Interval`]: each step's rounding enclosed): from its plane when `p`'s
/// foot there is inside it, or else from the nearest side. Where the enclosures cannot tell
/// whether the foot is inside, the hull of the two, since the true distance is one of them. Only
/// the sides the foot may be beyond are measured: a foot outside the triangle is nearest a point
/// of a side whose line it is beyond (the point's normal cone there leans out of that side). `None`
/// for a triangle that may have no area, which [`exact_reach`] decides.
#[allow(
    clippy::many_single_char_names,
    reason = "a point, the triangle's corners a, b and c and its normal n, as in the formulas"
)]
fn triangle_enclosed(p: [f64; 3], triangle: &Triangle) -> Result<Option<Interval>, NumError> {
    let &[a, b, c] = triangle;
    let (p, a, b, c) = (enclose(p)?, enclose(a)?, enclose(b)?, enclose(c)?);
    let (ab, bc, ca) = (less(b, a)?, less(c, b)?, less(a, c)?);
    let (ap, bp, cp) = (less(p, a)?, less(p, b)?, less(p, c)?);
    let n = outer(ab, less(c, a)?)?;
    let size = Interval::norm(&n)?;
    if size.lo() <= 0.0 {
        return Ok(None);
    }
    let turns = [
        inner(n, outer(ab, ap)?)?,
        inner(n, outer(bc, bp)?)?,
        inner(n, outer(ca, cp)?)?,
    ];
    let inside = turns.iter().all(|t| at_least_zero(*t));
    let plane = Interval::norm(&[inner(n, ap)?.div(size)?])?;
    if inside {
        return Ok(Some(plane));
    }
    let mut sides = None;
    for (turn, (v, e)) in turns.iter().zip([(ap, ab), (bp, bc), (cp, ca)]) {
        if matches!(turn.certain_sign(), Some(Sign::Positive)) {
            continue;
        }
        let Some(d) = segment_enclosed(v, e)? else {
            return Ok(None);
        };
        sides = Some(match sides {
            Some(s) => nearer(s, d)?,
            None => d,
        });
    }
    let Some(sides) = sides else {
        return Ok(None);
    };
    if turns
        .iter()
        .any(|t| matches!(t.certain_sign(), Some(Sign::Negative)))
    {
        return Ok(Some(sides));
    }
    hull(plane, sides).map(Some)
}

/// A vector of exact rationals.
type Exact = [RBig; 3];

#[allow(
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    reason = "exact rationals, which cannot overflow; two vectors and their coordinates"
)]
fn exact_less(a: &Exact, b: &Exact) -> Exact {
    let [x, y, z] = a;
    let [u, v, w] = b;
    [x - u, y - v, z - w]
}

#[allow(
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    reason = "exact rationals, which cannot overflow; two vectors and their coordinates"
)]
fn exact_inner(a: &Exact, b: &Exact) -> RBig {
    let [x, y, z] = a;
    let [u, v, w] = b;
    x * u + y * v + z * w
}

#[allow(
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    reason = "exact rationals, which cannot overflow; two vectors and their coordinates"
)]
fn exact_outer(a: &Exact, b: &Exact) -> Exact {
    let [x, y, z] = a;
    let [u, v, w] = b;
    [y * w - z * v, z * u - x * w, x * v - y * u]
}

/// The square of how far `v`, from a segment's start, is from the segment whose end is `e` from
/// its start, exactly.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "exact rationals, which cannot overflow; the divisor is shown nonzero"
)]
fn exact_segment(v: &Exact, e: &Exact) -> RBig {
    let (foot, length) = (exact_inner(v, e), exact_inner(e, e));
    if foot <= RBig::ZERO || length == RBig::ZERO {
        exact_inner(v, v)
    } else if foot >= length {
        let w = exact_less(v, e);
        exact_inner(&w, &w)
    } else {
        let x = exact_outer(v, e);
        exact_inner(&x, &x) / length
    }
}

/// The square of how far `p` is from `triangle`, exactly: every double is a rational, and the
/// rule is [`triangle_enclosed`]'s, decided without rounding. `None` if a coordinate is not
/// finite.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "exact rationals, which cannot overflow; the divisor is shown nonzero"
)]
#[allow(
    clippy::many_single_char_names,
    reason = "a point, the triangle's corners a, b and c and its normal n, as in the formulas"
)]
fn exact_squared(p: [f64; 3], triangle: &Triangle) -> Option<RBig> {
    let exact = |q: [f64; 3]| -> Option<Exact> {
        let [x, y, z] = q.map(|x| RBig::try_from(x).ok());
        Some([x?, y?, z?])
    };
    let &[a, b, c] = triangle;
    let (p, a, b, c) = (exact(p)?, exact(a)?, exact(b)?, exact(c)?);
    let (ab, bc, ca) = (exact_less(&b, &a), exact_less(&c, &b), exact_less(&a, &c));
    let (ap, bp, cp) = (exact_less(&p, &a), exact_less(&p, &b), exact_less(&p, &c));
    let n = exact_outer(&ab, &exact_less(&c, &a));
    let size = exact_inner(&n, &n);
    let inside = size > RBig::ZERO
        && [(&ab, &ap), (&bc, &bp), (&ca, &cp)]
            .iter()
            .all(|(e, v)| exact_inner(&n, &exact_outer(e, v)) >= RBig::ZERO);
    if inside {
        let height = exact_inner(&n, &ap);
        return Some(height.clone() * height / size);
    }
    [(&ap, &ab), (&bp, &bc), (&cp, &ca)]
        .iter()
        .map(|(v, e)| exact_segment(v, e))
        .min()
}

/// Bounds on how far `p` is from `triangle` from the exact square: the largest double whose square
/// is at most it, and the next double up, or that double twice when its square is it exactly.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "exact rationals, which cannot overflow"
)]
fn exact_reach(p: [f64; 3], triangle: &Triangle) -> (f64, f64) {
    let Some(square) = exact_squared(p, triangle) else {
        return (0.0, f64::INFINITY);
    };
    let squared = |x: f64| RBig::try_from(x).map(|r| r.clone() * r).ok();
    let below = |x: f64| squared(x).is_some_and(|s| s <= square);
    let mut root = square.to_f64().value().sqrt();
    while root > 0.0 && !below(root) {
        root = root.next_down();
    }
    while root.is_finite() && below(root.next_up()) {
        root = root.next_up();
    }
    if squared(root).is_some_and(|s| s == square) {
        (root, root)
    } else {
        (root, root.next_up())
    }
}

/// How far `p` is from `triangle`, certified: the lower bound is never above the true distance and
/// never more than [`CERTIFIED`], with [`WIDE`] of the distance, below it, and the upper bound is
/// never below it.
///
/// Why not the doubles alone: in doubles, closest-point methods take differences of products, and
/// on a sliver far from the origin those cancel (issue #951: a sample on its own triangle read
/// 3.03 steps away). So every quantity is measured from the triangle's own corners in intervals
/// (`geom_num::interval`, whose every step encloses its rounding), and where an enclosure is too
/// wide to decide or to bound the distance closely enough, in exact rationals, which cannot be
/// wrong.
fn reach(p: [f64; 3], triangle: &Triangle) -> (f64, f64) {
    if let Ok(Some(d)) = triangle_enclosed(p, triangle) {
        let (lo, hi) = (larger(d.lo(), 0.0), d.hi());
        if hi - lo <= CERTIFIED + hi * WIDE {
            return (lo, hi);
        }
    }
    exact_reach(p, triangle)
}

/// How far `p` is from `triangle`: [`reach`]'s lower bound, so a distance sampled from it can only
/// refute.
fn distance(p: [f64; 3], triangle: &Triangle) -> f64 {
    reach(p, triangle).0
}

/// A lower bound on how far `p` is from the box from `low` to `high`, certified by intervals: 0
/// where it cannot be told.
fn to_box(p: [f64; 3], [low, high]: &[[f64; 3]; 2]) -> f64 {
    let gap = |x: f64, l: f64, h: f64| -> Result<Interval, NumError> {
        if x < l {
            Interval::point(l)?.sub(Interval::point(x)?)
        } else if x > h {
            Interval::point(x)?.sub(Interval::point(h)?)
        } else {
            Interval::point(0.0)
        }
    };
    let [x, y, z] = p;
    let [lx, ly, lz] = *low;
    let [hx, hy, hz] = *high;
    [gap(x, lx, hx), gap(y, ly, hy), gap(z, lz, hz)]
        .into_iter()
        .collect::<Result<Vec<Interval>, NumError>>()
        .and_then(|g| Interval::norm(&g))
        .map_or(0.0, |d| larger(d.lo(), 0.0))
}

/// Whether the box from `low` to `high` is certainly farther from `p` than `highest`. First in
/// doubles, with a wide margin: the square of the gap takes at most eight rounded steps, each off
/// by at most a 2^-53 part of its value or the smallest subnormal, so a square in doubles over four
/// times `highest`'s square and one more square step is beyond it whatever the rounding. Between
/// that and `highest`, by [`to_box`]'s intervals.
fn beyond(p: [f64; 3], bounds: &[[f64; 3]; 2], highest: f64) -> bool {
    if !highest.is_finite() {
        return false;
    }
    let [low, high] = bounds;
    let mut plain = 0.0;
    for ((x, l), h) in p.iter().zip(low).zip(high) {
        let gap = if x < l {
            l - x
        } else if x > h {
            x - h
        } else {
            0.0
        };
        plain += gap * gap;
    }
    if plain > 4.0 * highest * highest + 1.0 {
        return true;
    }
    to_box(p, bounds) > highest
}

/// Triangles, each with its box, for the distance to the nearest.
struct Near<'t> {
    triangles: &'t [Triangle],
    boxes: Vec<[[f64; 3]; 2]>,
}

impl<'t> Near<'t> {
    fn new(triangles: &'t [Triangle]) -> Self {
        let boxes = triangles
            .iter()
            .map(|t| {
                let (mut low, mut high) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
                for corner in t {
                    for ((l, h), x) in low.iter_mut().zip(&mut high).zip(corner) {
                        *l = smaller(*l, *x);
                        *h = larger(*h, *x);
                    }
                }
                [low, high]
            })
            .collect();
        Near { triangles, boxes }
    }
}

/// Samples along each side of a triangle.
const DENSITY: u32 = 8;

/// The gap, in grid steps, sampling cannot see: on a sliver of a triangle a point's distance from
/// the triangle it came from rounds to about this, and it is far under anything that matters.
const SAMPLED: f64 = 1e-6;

/// Samples of each triangle of `triangles`: a lattice of `DENSITY` steps along two of its sides.
fn triangle_samples(triangles: &[Triangle]) -> Vec<[f64; 3]> {
    let mut out = Vec::new();
    let density = f64::from(DENSITY);
    for &[a, b, c] in triangles {
        let (u, v) = (sub(b, a), sub(c, a));
        for i in 0..=DENSITY {
            for j in 0..=DENSITY.saturating_sub(i) {
                out.push(along(
                    along(a, u, f64::from(i) / density),
                    v,
                    f64::from(j) / density,
                ));
            }
        }
    }
    out
}

/// The largest of `near` over `samples`.
fn farthest(samples: &[[f64; 3]], near: impl Fn([f64; 3]) -> f64) -> f64 {
    samples.iter().map(|&p| near(p)).fold(0.0, larger)
}

/// How far `p` is from the nearest of `near`'s triangles, by [`distance`]: a certified lower bound;
/// infinite if there are none. A triangle whose box is certainly farther than the nearest found so
/// far can be no nearer, and is passed by.
fn to_triangles(p: [f64; 3], near: &Near<'_>) -> f64 {
    let (mut lowest, mut highest) = (f64::INFINITY, f64::INFINITY);
    for (triangle, bounds) in near.triangles.iter().zip(&near.boxes) {
        if beyond(p, bounds, highest) {
            continue;
        }
        let (lo, hi) = reach(p, triangle);
        lowest = smaller(lowest, lo);
        highest = smaller(highest, hi);
    }
    lowest
}

/// The two-sided Hausdorff distance between two triangle sets, by sampling, so it can only refute.
/// Infinite if exactly one set is empty.
#[must_use]
pub fn hausdorff(a: &[Triangle], b: &[Triangle]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let (near_a, near_b) = (Near::new(a), Near::new(b));
    larger(
        farthest(&triangle_samples(a), |p| to_triangles(p, &near_b)),
        farthest(&triangle_samples(b), |p| to_triangles(p, &near_a)),
    )
}

/// How far `p` is from `triangle`, as the comparison measures it: for tests of the measure, which
/// is a certified lower bound on the true distance ([`distance`]).
#[must_use]
pub fn triangle_distance(p: [f64; 3], triangle: &Triangle) -> f64 {
    distance(p, triangle)
}

/// The square of how far `p` is from `triangle`, exactly, as a rational: for tests of
/// [`triangle_distance`]. `None` if a coordinate is not finite.
#[must_use]
pub fn triangle_distance_squared(p: [f64; 3], triangle: &Triangle) -> Option<RBig> {
    exact_squared(p, triangle)
}

/// One face of the kernel's result, each corner the double nearest its exact place.
struct Face {
    /// The unit normal of its plane.
    normal: [f64; 3],
    /// Where its plane is: `normal · x + offset` is 0 on it.
    offset: f64,
    /// The two axes of the plan it is drawn in, the axes its normal leans on least, and the axis it
    /// is lifted along.
    axes: [usize; 3],
    /// Its loops, the outer first.
    rings: Vec<Vec<[f64; 3]>>,
}

/// Coordinate `axis` of `p`.
fn coordinate_of(p: [f64; 3], axis: usize) -> f64 {
    p.get(axis).copied().unwrap_or(0.0)
}

/// Each side of each loop of `face`, from a corner to the next.
fn sides_of(face: &Face) -> impl Iterator<Item = ([f64; 3], [f64; 3])> + '_ {
    face.rings.iter().flat_map(|ring| {
        let after = ring.iter().cycle().skip(1);
        ring.iter().copied().zip(after.copied())
    })
}

/// Whether `p`, a point of `face`'s plane, is inside its loops: whether a ray from it along the
/// plan's first axis crosses their sides an odd number of times (the crossing test).
#[allow(
    clippy::many_single_char_names,
    reason = "the plan's axes i, j and k and a point's x and y, as in the crossing test"
)]
fn encloses(face: &Face, p: [f64; 3]) -> bool {
    let [i, j, _] = face.axes;
    let (x, y) = (coordinate_of(p, i), coordinate_of(p, j));
    let mut inside = false;
    for (a, b) in sides_of(face) {
        let (ax, ay) = (coordinate_of(a, i), coordinate_of(a, j));
        let (bx, by) = (coordinate_of(b, i), coordinate_of(b, j));
        if (ay > y) != (by > y) && x < ax + (y - ay) * (bx - ax) / (by - ay) {
            inside = !inside;
        }
    }
    inside
}

/// How far `p` is from the segment from `a` to `b`: Ericson's section 5.1.2.
fn to_segment(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let ab = sub(b, a);
    let length = dot(ab, ab);
    let t = if length > 0.0 {
        larger(0.0, smaller(1.0, dot(sub(p, a), ab) / length))
    } else {
        0.0
    };
    let gap = sub(p, along(a, ab, t));
    dot(gap, gap).sqrt()
}

/// How far `p` is from `face`: from its plane if its foot there is inside the face, or else from
/// the nearest side.
fn to_face(p: [f64; 3], face: &Face) -> f64 {
    let height = dot(face.normal, p) + face.offset;
    if encloses(face, along(p, face.normal, -height)) {
        return height.abs();
    }
    sides_of(face)
        .map(|(a, b)| to_segment(p, a, b))
        .fold(f64::INFINITY, smaller)
}

/// How far `p` is from the nearest of `faces`; infinite if there are none.
fn to_faces(p: [f64; 3], faces: &[Face]) -> f64 {
    faces
        .iter()
        .map(|f| to_face(p, f))
        .fold(f64::INFINITY, smaller)
}

/// Samples of each face of `faces`: `DENSITY` steps along each side, and a lattice of `DENSITY`
/// steps each way over its box in the plan, lifted to its plane where it is inside the face.
#[allow(
    clippy::many_single_char_names,
    reason = "the plan's axes i, j and k and a point's x and y, as in the crossing test"
)]
fn face_samples(faces: &[Face]) -> Vec<[f64; 3]> {
    let mut out = Vec::new();
    let density = f64::from(DENSITY);
    for face in faces {
        let [i, j, k] = face.axes;
        let (mut low, mut high) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for (a, b) in sides_of(face) {
            for s in 0..=DENSITY {
                out.push(along(a, sub(b, a), f64::from(s) / density));
            }
            for ((l, h), axis) in low.iter_mut().zip(&mut high).zip([i, j]) {
                *l = smaller(*l, coordinate_of(a, axis));
                *h = larger(*h, coordinate_of(a, axis));
            }
        }
        let [low_x, low_y] = low;
        let [high_x, high_y] = high;
        let lean = coordinate_of(face.normal, k);
        for s in 0..=DENSITY {
            for t in 0..=DENSITY {
                let x = low_x + (high_x - low_x) * f64::from(s) / density;
                let y = low_y + (high_y - low_y) * f64::from(t) / density;
                let lifted = -(face.offset
                    + coordinate_of(face.normal, i) * x
                    + coordinate_of(face.normal, j) * y)
                    / lean;
                let mut p = [0.0; 3];
                for (axis, value) in [(i, x), (j, y), (k, lifted)] {
                    if let Some(slot) = p.get_mut(axis) {
                        *slot = value;
                    }
                }
                if encloses(face, p) {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// The two-sided Hausdorff distance between the kernel's exact `faces` and an oracle's
/// `triangles`, by sampling both, so it can only refute. Infinite if exactly one side is empty.
fn hausdorff_exact(faces: &[Face], triangles: &[Triangle]) -> f64 {
    if faces.is_empty() && triangles.is_empty() {
        return 0.0;
    }
    let near = Near::new(triangles);
    larger(
        farthest(&face_samples(faces), |p| to_triangles(p, &near)),
        farthest(&triangle_samples(triangles), |p| to_faces(p, faces)),
    )
}

/// Each face of `mesh` from its exact `corners`.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "doubles, which cannot overflow here"
)]
fn exact_faces(mesh: &ExactMesh, corners: &[[RBig; 3]]) -> Result<Vec<Face>, String> {
    let parts = mesh.parts();
    let mut out = Vec::new();
    for (rings, facing) in faces_of(mesh)?.into_iter().zip(&parts.face_plane) {
        let plane = parts
            .planes
            .get(facing.row as usize)
            .ok_or_else(|| "a plane past the pool".to_owned())?;
        let double = |x: i128| RBig::from(x).to_f64().value();
        let n = plane.n().map(double);
        let size = dot(n, n).sqrt();
        let normal = n.map(|x| x / size);
        let [a, b, c] = n.map(f64::abs);
        let axes = if a >= b && a >= c {
            [1, 2, 0]
        } else if b >= c {
            [2, 0, 1]
        } else {
            [0, 1, 2]
        };
        let mut loops = Vec::with_capacity(rings.len());
        for ring in rings {
            let mut points = Vec::with_capacity(ring.len());
            for v in ring {
                points.push(row(corners, Some(v))?.clone().map(|x| x.to_f64().value()));
            }
            loops.push(points);
        }
        out.push(Face {
            normal,
            offset: double(plane.d()) / size,
            axes,
            rings: loops,
        });
    }
    Ok(out)
}

/// What the kernel made of a job, as a comparison reads it: its exact volume, its area and the
/// length of its loops from its exact corners, and its faces, each corner the double nearest its
/// exact place, not the grid point the export would write.
struct Made {
    volume: RBig,
    area: f64,
    edges: f64,
    faces: Vec<Face>,
}

fn made(mesh: &ExactMesh) -> Result<Made, String> {
    let corners = corners_of(mesh)?;
    let (area, edges) = sizes(mesh)?;
    Ok(Made {
        volume: volume(mesh)?,
        area,
        edges,
        faces: exact_faces(mesh, &corners)?,
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

/// The exact reference's volume for `job`, written from the definitions alone: the volume where
/// the two solids' winding numbers keep the job's boolean, by
/// [`flat_reference::six_volume_where`], which cuts only the faces and so serves the stored models
/// (at most about 20 s on one of them, optimised). The arrangement of space
/// ([`flat_reference::boolean`]) cuts space by every plane: on the union of a model of 212
/// triangles with its moved copy it was still running after 11 minutes, where this takes 2 s. The
/// reference's own tests hold the two to one number.
///
/// # Errors
///
/// A job the reference cannot read.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "exact rationals, which cannot overflow"
)]
pub fn reference(job: &Job) -> Result<Q, String> {
    let (a, b) = (reference_solid(&job.a)?, reference_solid(&job.b)?);
    let op = job.op;
    let keep = |w: &[i64]| {
        let (a, b) = match w {
            &[a, b] => (a > 0, b > 0),
            _ => (false, false),
        };
        match op {
            Op::Union => a || b,
            Op::Intersection => a && b,
            Op::Difference => a && !b,
        }
    };
    flat_reference::six_volume_where(&[&a, &b], keep)
        .map(|six| six / Q::from(6))
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
    let stated = larger(double(answer.tolerance.as_ref(), "tolerance", job.id)?, 0.0);
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
    let by_volume = allowance(larger(our_area, area), 0.0, our_volume.abs() * f64::EPSILON);
    let by_area = allowance(edges, 0.0, our_area * f64::EPSILON * 16.0);
    // Measured from the exact faces: nothing of ours is rounded.
    let by_gap = allowance(1.0, 0.0, SAMPLED);
    let kind = if !within(our_volume, theirs, &by_volume) {
        "volume"
    } else if !within(our_area, area, &by_area) {
        "area"
    } else if hausdorff_exact(&ours.faces, &shape) > by_gap.total() {
        "hausdorff"
    } else {
        return Ok(Compared::Agree);
    };
    Ok(difference(oracle, job, kind, settle(job, &ours.volume)?))
}

/// Where `oracle`'s own documentation says it does not take a surface that crosses itself, or
/// `None` for an oracle whose documentation says nothing of it, which is judged on such a job as
/// on any other.
#[must_use]
pub fn stated_input(oracle: &str) -> Option<&'static str> {
    match oracle {
        MANIFOLD => Some(
            "Manifold's documentation (its wiki, page \"Manifold Library\", section \"Overlaps\", \
             https://github.com/elalish/manifold/wiki/Manifold-Library) promises a correct result \
             only from input that does not overlap itself (epsilon-valid), and says that from a \
             self-overlapping input mesh its triangulator may build output surfaces incorrectly",
        ),
        CGAL => Some(
            "CGAL's user manual (6.0.1, Polygon Mesh Processing, \"Corefinement and Boolean \
             Operations\", https://doc.cgal.org/6.0.1/Polygon_mesh_processing/index.html) states \
             that the input meshes of its booleans must not self-intersect, and its Nef \
             polyhedron from a polyhedral surface (Nef_3) is documented only as the point set \
             that surface represents",
        ),
        _ => None,
    }
}

/// Whether `solid` crosses itself, by the kernel's own exact reading of winding numbers
/// ([`flat_reference::six_volume_where`], chapters 11.1 and 19.5): whether a region of positive
/// volume has a winding number other than 0 and 1, an overlap (2 or more) or a pocket of the
/// surface turned inside out (below 0). A surface whose sheets only touch has winding numbers 0
/// and 1 alone, and does not cross itself by this reading. Never a hand-made mark: the solid as the
/// oracle receives it, on the grid, decides.
///
/// # Errors
///
/// A solid the reference cannot read.
pub fn crosses(solid: &Solid) -> Result<bool, String> {
    let read = reference_solid(solid)?;
    let six = flat_reference::six_volume_where(&[&read], |w| !matches!(w, [0 | 1]))
        .map_err(|e| format!("the reference: {e:?}"))?;
    Ok(six != Q::ZERO)
}

/// `solid` moved so its first corner is the origin: a solid's winding numbers move with it, so
/// whether it crosses itself is the moved one's.
fn placed(solid: &Solid) -> Solid {
    let first = solid.vertices.first().copied().unwrap_or_default();
    Solid {
        vertices: solid.vertices.iter().map(|&v| minus(first, v)).collect(),
        triangles: solid.triangles.clone(),
    }
}

/// `job` judged by the exact reference alone, its model outside `oracle`'s stated input
/// (`stated`), from the kernel's exact volume `ours` (or its refusal): [`Compared::Outside`] when
/// that is the reference's volume, and otherwise a difference of kind `reference`, which fails
/// the run like any other. The oracle's answer is not read.
///
/// # Errors
///
/// A job the reference cannot read.
pub fn judge_by_reference(
    oracle: &str,
    job: &Job,
    stated: &str,
    ours: Result<RBig, String>,
) -> Result<Compared, String> {
    let volume = match ours {
        Ok(volume) => volume,
        Err(why) => {
            return Ok(difference(
                oracle,
                job,
                "reference",
                format!("ours: the kernel refuses a model that crosses itself: {why}"),
            ));
        }
    };
    if reference(job)? != volume {
        return Ok(difference(
            oracle,
            job,
            "reference",
            "ours: the kernel's volume is not the exact reference's, on a model that crosses \
             itself"
                .to_owned(),
        ));
    }
    Ok(Compared::Outside {
        why: format!(
            "a model crosses itself, outside {oracle}'s stated input: {stated}. Judged by the \
             exact reference alone: the kernel's volume is the reference's"
        ),
    })
}

/// The comparison's judge, which remembers which solids cross themselves, so a model's three jobs
/// read its winding numbers once.
#[derive(Debug, Default)]
pub struct Judge {
    crossing: BTreeMap<Solid, bool>,
}

impl Judge {
    /// A judge that has read no solid yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether `solid` crosses itself ([`crosses`]). A solid off the grid, which the kernel cannot
    /// read and rightly refuses, or one the reference cannot read, does not, and is judged as any
    /// other.
    fn crossing(&mut self, solid: &Solid) -> bool {
        if solid
            .vertices
            .iter()
            .any(|&[x, y, z]| P3::new(x, y, z).is_err())
        {
            return false;
        }
        let key = placed(solid);
        if let Some(known) = self.crossing.get(&key) {
            return *known;
        }
        let found = crosses(&key).unwrap_or(false);
        self.crossing.insert(key, found);
        found
    }

    /// Judges one answer of `oracle`. When a solid of `job` crosses itself ([`crosses`]) and
    /// `oracle`'s documentation says it does not take such a surface ([`stated_input`]), the job is
    /// judged by the exact reference alone ([`judge_by_reference`]); otherwise the answer is
    /// compared with the kernel's result.
    ///
    /// # Errors
    ///
    /// A job or answer that cannot be read: the harness is broken, not the kernel.
    pub fn judge(&mut self, oracle: &str, job: &Job, answer: &Answer) -> Result<Compared, String> {
        if job.id != answer.id {
            return Err(format!("answer {} for job {}", answer.id, job.id));
        }
        match stated_input(oracle) {
            Some(stated) if self.crossing(&job.a) || self.crossing(&job.b) => {
                let ours = run(job).and_then(|mesh| volume(&mesh));
                judge_by_reference(oracle, job, stated, ours)
            }
            _ => compared(oracle, job, answer),
        }
    }
}

/// Judges one answer of `oracle`, as a fresh [`Judge`] does.
///
/// # Errors
///
/// A job or answer that cannot be read: the harness is broken, not the kernel.
pub fn judge(oracle: &str, job: &Job, answer: &Answer) -> Result<Compared, String> {
    Judge::new().judge(oracle, job, answer)
}

/// `answer` compared with the kernel's result for `job`.
fn compared(oracle: &str, job: &Job, answer: &Answer) -> Result<Compared, String> {
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
    if !is_exact(oracle) {
        return match made(mesh) {
            Ok(made) => floating(oracle, job, &made, answer),
            Err(why) => {
                let settled = settle(job, &volume(mesh)?)?;
                Ok(difference(
                    oracle,
                    job,
                    "unmeasured",
                    format!("{settled}; its result cannot be measured: {why}"),
                ))
            }
        };
    }
    let ours = volume(mesh)?;
    let theirs: RBig = theirs
        .parse()
        .map_err(|e| format!("job {}: the volume {theirs:?}: {e:?}", job.id))?;
    if exact(&ours, &theirs) {
        return Ok(Compared::Agree);
    }
    Ok(difference(oracle, job, "volume", settle(job, &ours)?))
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

/// Runs an operation [`operation`] wrote; `None` for any other operation. Its two bodies are the
/// hashes of the result's exact volume and of its triangles as the export makes them (or the
/// export's refusal), so a triaged case's fixture holds both.
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

/// The SHA-256 of what the export makes of `mesh` in triangles: each vertex and each triangle with
/// its face, in order, as text, or the refusal.
fn exported(mesh: &ExactMesh) -> [u8; 32] {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let mut sha = Sha256::new();
    match triangles(mesh, &mut meter) {
        Ok(drawn) => {
            for p in &drawn.vertices {
                sha.write(format!("v {} {} {}\n", p.x.get(), p.y.get(), p.z.get()).as_bytes());
            }
            for (&[a, b, c], face) in drawn.corners.iter().zip(&drawn.faces) {
                sha.write(format!("t {a} {b} {c} {face}\n").as_bytes());
            }
        }
        Err(e) => sha.write(format!("refused {e:?}\n").as_bytes()),
    }
    sha.finish()
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
        bodies: vec![sha.finish(), exported(&mesh)],
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
    let mesh = run(job)?;
    let ours = made(&mesh)?;
    let corners = corners_of(&mesh)?;
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let drawn = triangles(&mesh, &mut meter).map_err(|e| format!("{e:?}"))?;
    let place = |v: u32| -> Result<[f64; 3], String> {
        Ok(row(&corners, Some(&v))?.clone().map(|x| x.to_f64().value()))
    };
    let mut shape = Vec::with_capacity(drawn.corners.len());
    for &[a, b, c] in &drawn.corners {
        shape.push([place(a)?, place(b)?, place(c)?]);
    }
    let text = |x: f64| float(x).unwrap_or_default();
    // The export's triangles, split on corners rounded to the grid and drawn between the exact
    // ones, so where a corner was rounded one may stray from its face by up to a step.
    let stated = if drawn.rounded { 1.0 } else { 0.0 };
    Ok(Answer {
        id: job.id,
        volume: Some(text(ours.volume.to_f64().value())),
        area: Some(text(ours.area)),
        tolerance: Some(text(stated)),
        triangles: Some(shape.iter().map(|t| t.map(|p| p.map(text))).collect()),
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
