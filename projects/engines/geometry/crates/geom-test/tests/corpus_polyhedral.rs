// SPDX-License-Identifier: Apache-2.0
//! The polyhedral corpus of 11.17's item 15 that random grid boxes leave out, held exactly to the
//! rational reference (Gate 4, G156; G140's done-when: "EMBER figures, Manifold regressions").
//!
//! - **EMBER's figures 24 and 25** (`geom_test::polyhedral`), every selection a solid can be read
//!   by in one arrangement: union, intersection, the first less the rest, inside at least two,
//!   inside exactly one, imprint, split, general fuse, cells and section. Each result's volume is
//!   exactly the unit cells it keeps and exactly the flat reference's
//!   (`flat_reference::six_volume_where`, by the operands' winding numbers), each cell holds the
//!   unit cells of its own operands and no others, and every body passes both checkers of 28.4,
//!   which agree.
//! - **Manifold's regression models** (the stored corpus `manifold-models`, Apache-2.0, read onto
//!   the grid by `geom_test::obj` at the scale and rounding it states): each united alone and with
//!   a copy moved a third of its size along each axis by `geom-mesh`'s boolean, its result's exact
//!   volume (computed here from its planes and corners, in rationals) the reference's, and inside
//!   the kernel's own exact or certified volume. A model the mesh builder refuses is named with its
//!   refusal in [`REFUSED`].
//! - **Exact transforms commute** with every boolean on both: a grid translation, a quarter turn
//!   and a mirror give the same volume and the same corners, moved.
//! - **The same bytes everywhere**: every result's records in one digest, the golden
//!   `geom-test/polyhedral-corpus`, checked on every target by the determinism matrix, and on one
//!   thread and every pool of `golden::THREAD_COUNTS` threads.
//!
//! The second checker of 28.4 says how many faces and edges it cannot judge: on this corpus none,
//! every face being flat and every edge straight (`UNJUDGED`).
//!
//! Tiers: the reference's rationals and the transforms take minutes in a debug build, so those
//! tests run in every release leg; the golden runs everywhere. The models of more than `LARGE`
//! faces run in the nightly tier (`kernel-nightly.yml`), with `--ignored`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::large_enum_variant,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::unwrap_used,
    reason = "tests: native arithmetic, threads for chapter 21's thread counts, and a failure stops the test"
)]

use std::collections::{BTreeMap, BTreeSet};

use dashu_int::IBig;
use dashu_ratio::RBig;
use geom_bool::combine::{Combined, combine};
use geom_bool::operand::{BooleanInput, Operand, OperandKey, OperandKind, Selection, Term};
use geom_format::body::write_body;
use geom_make::{BoxInput, Moved, make_box, mirror, move_by, rotate};
use geom_math::{Axis, Rotation};
use geom_mesh::boolean::boolean;
use geom_mesh::exact::build::build;
use geom_mesh::exact::mass::Volume;
use geom_mesh::exact::{ExactMesh, Facing, Vertex};
use geom_num::{Exactness, P3, Steps};
use geom_op::{OpContext, OpId, OpPath};
use geom_poly::merge::Merge;
use geom_poly::winding::WindingTest;
use geom_run::{Budget, Exec, Pool, WorkMeter};
use geom_test::agree::judge;
use geom_test::flat_reference::{Q, Solid, int, six_volume_where, six_volumes_where};
use geom_test::golden::{self, Sha256, hex};
use geom_test::obj::{self, Grid};
use geom_test::polyhedral::{
    Block, MODELS, box_solid, figure_24, figure_25, mesh_polygons, mesh_solid, unit_cells,
};
use geom_topo::query::{corner, point as vertex_point, shell_volume, volume};
use geom_topo::{Body, BodyKind, FaceIx, Sense, ShellIx, VertexIx};

#[path = "catalogue/mod.rs"]
mod catalogue;

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-test/polyhedral-corpus";

/// The faces and edges of the corpus the second checker cannot judge: every face is flat and every
/// edge straight, so none (stated for Gate 4's record).
const UNJUDGED: usize = 0;

/// Each model of the pull-request tier `geom-mesh`'s builder refuses once on the grid, with the
/// refusal: a face whose corners round onto one line, and an edge met by an odd number of faces.
/// Each is the builder's typed refusal of a mesh that is not closed and manifold on the grid, whose
/// remedy is healing (19.2), not a fault.
const REFUSED: [(&str, &str); 4] = [
    (
        "dedupe_stale_duplicate",
        "Fault(DegenerateFace { face: 0 })",
    ),
    (
        "degenerate_split_quad_first",
        "Fault(DegenerateFace { face: 51 })",
    ),
    (
        "degenerate_swap_cycle_first",
        "Fault(UnevenEdge { half_edge: 227 })",
    ),
    (
        "openscad-nonmanifold-crash",
        "Fault(DegenerateFace { face: 1359 })",
    ),
];

/// As [`REFUSED`], for the nightly tier.
const REFUSED_LARGE: [(&str, &str); 1] = [("Offset1", "Fault(DegenerateFace { face: 1010 })")];

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([9; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// A pool of `n` threads: each takes one run of the tasks, in order. Each call starts its own
/// threads, so a task that calls the helper again makes progress.
struct Threads(usize);

impl Pool for Threads {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        let run = tasks.len().div_ceil(self.0.max(1)).max(1);
        std::thread::scope(|scope| {
            for lane in tasks.chunks_mut(run) {
                scope.spawn(move || {
                    for task in lane {
                        task();
                    }
                });
            }
        });
    }
}

fn key(i: usize) -> OperandKey {
    OperandKey([u8::try_from(i + 1).unwrap(); 16])
}

fn solid(i: usize, body: &Body) -> Operand<'_> {
    Operand {
        key: key(i),
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

fn block_body(b: &Block) -> Body {
    let input = BoxInput {
        min: P3::new(b.low[0], b.low[1], b.low[2]).unwrap(),
        size: [0, 1, 2].map(|k| Steps((b.high[k] - b.low[k]) as u64)),
    };
    make_box(&ctx(), &input).unwrap().result.body
}

/// Six times a body's volume: a general body's is its shells', as a regularised boolean bounds its
/// material by closed shells even where it is not manifold; no body at all has none.
fn six(body: &Body) -> i128 {
    if body.arenas().edges.is_empty() {
        return 0;
    }
    let exact = exact_six_of(body);
    let grid = if body.kind() == BodyKind::General {
        (0..body.arenas().shells.len())
            .map(|row| {
                let s = ShellIx::new(row).unwrap();
                shell_volume(body, s, &mut meter()).map(|v| v.to_i128().unwrap())
            })
            .sum::<Result<i128, _>>()
    } else {
        volume(body, &mut meter()).map(|v| v.to_i128().unwrap())
    };
    // A vertex where three planes meet has no grid volume: then the volume in rationals, a whole
    // number of sixths here.
    let Ok(v) = grid else {
        assert_eq!(exact.denominator(), &dashu_int::UBig::ONE, "{exact}");
        return i128::try_from(exact.numerator().clone()).unwrap();
    };
    assert_eq!(
        RBig::from(IBig::from(v)),
        exact,
        "the kernel's volume and the exact one"
    );
    v
}

/// Six times a body's volume in exact rationals, from every vertex's exact place: over every face
/// of every shell, twice its area vector (each loop's corners in ring order) dotted with a vertex
/// of it, as `geom_topo::query::shell_volume` sums it on the grid.
fn exact_six_of(body: &Body) -> RBig {
    let a = body.arenas();
    let at =
        |v: VertexIx| -> [RBig; 3] { catalogue::flat::exact(&corner(body, v).unwrap()).unwrap() };
    let mut sum = RBig::ZERO;
    for row in 0..a.faces.len() {
        let face = a.faces.get(FaceIx::new(row).unwrap()).unwrap();
        let mut area = [RBig::ZERO, RBig::ZERO, RBig::ZERO];
        let mut first: Option<[RBig; 3]> = None;
        let mut next_loop = face.first_loop;
        while let Some(l) = next_loop {
            let ring = a.loops.get(l).unwrap();
            let start = ring.first_fin.unwrap();
            let mut fin = start;
            let mut points = Vec::new();
            loop {
                let f = a.fins.get(fin).unwrap();
                let e = a.edges.get(f.edge).unwrap();
                let v = if f.sense == Sense::Forward {
                    e.start
                } else {
                    e.end
                };
                points.push(at(v.unwrap()));
                fin = f.next;
                if fin == start {
                    break;
                }
            }
            for (p, q) in points.iter().zip(points.iter().cycle().skip(1)) {
                area[0] += &p[1] * &q[2] - &p[2] * &q[1];
                area[1] += &p[2] * &q[0] - &p[0] * &q[2];
                area[2] += &p[0] * &q[1] - &p[1] * &q[0];
            }
            first = first.or_else(|| points.first().cloned());
            next_loop = ring.next;
        }
        let p = first.unwrap();
        sum += &area[0] * &p[0] + &area[1] * &p[1] + &area[2] * &p[2];
    }
    sum
}

/// Every grid corner of a body.
fn corners(body: &Body) -> BTreeSet<[i64; 3]> {
    (0..body.arenas().vertices.len())
        .filter_map(VertexIx::new)
        .filter_map(|v| vertex_point(body, v).ok())
        .map(|q| [q.x.get(), q.y.get(), q.z.get()])
        .collect()
}

/// A selection of a figure, and what it keeps of a unit cell from which boxes hold the cell:
/// `None` for the section, which has no volume.
struct Pick {
    name: &'static str,
    select: Selection,
    keep: Option<fn(&[bool]) -> bool>,
}

fn count(held: &[bool]) -> usize {
    held.iter().filter(|h| **h).count()
}

/// Every selection a solid's arrangement answers, the first box the target.
fn picks() -> Vec<Pick> {
    let target = key(0);
    vec![
        Pick {
            name: "union",
            select: Selection::Unite,
            keep: Some(|h| count(h) > 0),
        },
        Pick {
            name: "intersection",
            select: Selection::Intersect,
            keep: Some(|h| count(h) == h.len()),
        },
        Pick {
            name: "the first less the rest",
            select: Selection::Subtract { target },
            keep: Some(|h| h[0] && count(h) == 1),
        },
        Pick {
            name: "inside at least two",
            select: Selection::Where(vec![Term::AtLeast(2)]),
            keep: Some(|h| count(h) >= 2),
        },
        Pick {
            name: "inside exactly one",
            select: Selection::Where(vec![
                Term::AtLeast(1),
                Term::AtLeast(2),
                Term::Not,
                Term::And,
            ]),
            keep: Some(|h| count(h) == 1),
        },
        Pick {
            name: "imprint",
            select: Selection::Imprint { target },
            keep: Some(|h| h[0]),
        },
        Pick {
            name: "split",
            select: Selection::Split { target },
            keep: Some(|h| h[0]),
        },
        Pick {
            name: "general fuse",
            select: Selection::GeneralFuse,
            keep: Some(|h| count(h) > 0),
        },
        Pick {
            name: "cells",
            select: Selection::Cells {
                test: vec![Term::AtLeast(1)],
                seeds: Vec::new(),
            },
            keep: Some(|h| count(h) > 0),
        },
        Pick {
            name: "section",
            select: Selection::Section,
            keep: None,
        },
    ]
}

/// Every selection of `blocks` in one arrangement, on `exec`.
fn select_all(blocks: &[Block], exec: &Exec<'_>) -> Vec<Vec<Combined>> {
    let bodies: Vec<Body> = blocks.iter().map(block_body).collect();
    let input = BooleanInput {
        operands: bodies
            .iter()
            .enumerate()
            .map(|(i, b)| solid(i, b))
            .collect(),
        select: picks().into_iter().map(|p| p.select).collect(),
        merge: Merge::RemoveRedundant,
        accept: None,
    };
    combine(&ctx(), exec, &input).unwrap().result
}

/// The reference's six times the volume each of `keeps` keeps of `blocks`.
fn reference_six(blocks: &[Block], keeps: &[fn(&[bool]) -> bool]) -> Vec<Q> {
    let solids: Vec<Solid> = blocks.iter().map(|b| box_solid(b).unwrap()).collect();
    let refs: Vec<&Solid> = solids.iter().collect();
    let tests: Vec<Box<dyn Fn(&[i64]) -> bool>> = keeps
        .iter()
        .map(|keep| {
            let keep = *keep;
            Box::new(move |w: &[i64]| {
                let held: Vec<bool> = w.iter().map(|x| *x > 0).collect();
                keep(&held)
            }) as Box<dyn Fn(&[i64]) -> bool>
        })
        .collect();
    let borrowed: Vec<&dyn Fn(&[i64]) -> bool> = tests.iter().map(|t| t.as_ref()).collect();
    six_volumes_where(&refs, &borrowed).unwrap()
}

/// Both checkers on every body, adding up what the second could not judge.
fn both_pass(what: &str, bodies: &[Combined], unjudged: &mut usize) {
    for (k, one) in bodies.iter().enumerate() {
        let v = judge(&one.body, Exactness::Exact).unwrap();
        assert!(v.agree() && v.passed(), "{what} {k}: {v:?}");
        *unjudged += v.unjudged;
    }
}

fn figures() -> [(&'static str, Vec<Block>); 2] {
    [("figure 24", figure_24()), ("figure 25", figure_25())]
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "the reference over 40 beams takes minutes in a debug build; every release leg runs it"
)]
fn every_selection_of_ember_s_figures_is_the_unit_cells_and_the_reference() {
    let mut unjudged = 0;
    for (figure, blocks) in figures() {
        let results = select_all(&blocks, &Exec::serial());
        let keeps: Vec<fn(&[bool]) -> bool> = picks().iter().filter_map(|p| p.keep).collect();
        let mut references = reference_six(&blocks, &keeps).into_iter();
        for (pick, bodies) in picks().iter().zip(&results) {
            let what = format!("{figure}, {}", pick.name);
            both_pass(&what, bodies, &mut unjudged);
            let Some(keep) = pick.keep else {
                // A section is the curves where the boxes' faces meet, as a wire.
                assert!(
                    bodies.iter().any(|b| !b.body.arenas().edges.is_empty()),
                    "{what}"
                );
                continue;
            };
            let got: i128 = bodies.iter().map(|b| six(&b.body)).sum();
            let cells = unit_cells(&blocks, keep);
            assert_eq!(got, 6 * cells, "{what}: kernel against the unit cells");
            assert_eq!(
                Some(Q::from(IBig::from(got))),
                references.next(),
                "{what}: kernel against the reference"
            );
        }
        // Each cell holds exactly the unit cells of the boxes it says it lies in.
        let cells = &results[8];
        let mut by_boxes: BTreeMap<Vec<OperandKey>, i128> = BTreeMap::new();
        for cell in cells {
            let mut around = cell.cell.as_ref().unwrap().elements.clone();
            around.sort_unstable();
            *by_boxes.entry(around).or_insert(0) += six(&cell.body);
        }
        for (around, got) in &by_boxes {
            let want = unit_cells(&blocks, |held| {
                held.iter()
                    .enumerate()
                    .all(|(i, h)| *h == around.contains(&key(i)))
            });
            assert_eq!(*got, 6 * want, "{figure}: the cell inside {around:?}");
        }
        let covered: i128 = by_boxes.values().sum();
        assert_eq!(
            covered,
            6 * unit_cells(&blocks, |h| count(h) > 0),
            "{figure}"
        );
    }
    assert_eq!(unjudged, UNJUDGED);
}

#[test]
fn every_figure_result_is_written_in_the_native_format() {
    // Every result of every selection is a body the native format stores (24.3), and has no
    // vertex the subdivision's boxes made: none on a plane that halves a grid step (11.3: the
    // same result however the boxes fell). Imprint and general fuse fail both today (see the
    // pull request): a seam their cuts hold open keeps a vertex where a box's plane crosses it,
    // named by unreduced planes, which the format refuses.
    let mut wrong = Vec::new();
    for (figure, blocks) in figures() {
        for (pick, bodies) in picks().iter().zip(select_all(&blocks, &Exec::serial())) {
            for (k, one) in bodies.iter().enumerate() {
                if let Err(why) = write_body(&one.body, &mut meter()) {
                    wrong.push(format!("{figure}, {} {k}: {why:?}", pick.name));
                }
                let a = one.body.arenas();
                for row in 0..a.vertices.len() {
                    let v = VertexIx::new(row).unwrap();
                    let [x, y, z] = catalogue::flat::exact(&corner(&one.body, v).unwrap()).unwrap();
                    if [x, y, z]
                        .iter()
                        .any(|c| c.denominator() != &dashu_int::UBig::ONE)
                    {
                        wrong.push(format!(
                            "{figure}, {} {k}: a vertex off the grid",
                            pick.name
                        ));
                        break;
                    }
                }
            }
        }
    }
    assert_eq!(wrong, Vec::<String>::new());
}

/// An exact transform: a grid translation, a quarter turn about `z` or a mirror in `x`, both by
/// the kernel's transform operations on a body and by hand on a point.
#[derive(Clone, Copy, Debug)]
enum Move {
    Shift([i64; 3]),
    Turn,
    Mirror,
}

impl Move {
    fn body(self, b: &Body) -> Body {
        let moved = Moved {
            body: b,
            exactness: Exactness::Exact,
            accept: None,
        };
        let origin = P3::new(0, 0, 0).unwrap();
        match self {
            Move::Shift(by) => move_by(&ctx(), moved, by),
            Move::Turn => rotate(&ctx(), moved, Axis::Z, Rotation::quarter_turns(1), origin),
            Move::Mirror => mirror(&ctx(), moved, Axis::X, origin),
        }
        .unwrap()
        .result
        .body
    }

    fn point(self, [x, y, z]: [i64; 3]) -> [i64; 3] {
        match self {
            Move::Shift([a, b, c]) => [x + a, y + b, z + c],
            Move::Turn => [-y, x, z],
            Move::Mirror => [-x, y, z],
        }
    }

    /// A rational point moved.
    fn exact(self, [x, y, z]: [RBig; 3]) -> [RBig; 3] {
        match self {
            Move::Shift([a, b, c]) => [x + int(a), y + int(b), z + int(c)],
            Move::Turn => [-y, x, z],
            Move::Mirror => [-x, y, z],
        }
    }
}

const MOVES: [Move; 3] = [Move::Shift([7, -5, 3]), Move::Turn, Move::Mirror];

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "three transforms of every selection take minutes in a debug build; every release leg runs it"
)]
fn ember_s_figures_commute_with_exact_transforms() {
    for (figure, blocks) in figures() {
        let bodies: Vec<Body> = blocks.iter().map(block_body).collect();
        let three = |bodies: &[Body]| {
            let input = BooleanInput {
                operands: bodies
                    .iter()
                    .enumerate()
                    .map(|(i, b)| solid(i, b))
                    .collect(),
                select: vec![
                    Selection::Unite,
                    Selection::Intersect,
                    Selection::Subtract { target: key(0) },
                ],
                merge: Merge::RemoveRedundant,
                accept: None,
            };
            combine(&ctx(), &Exec::serial(), &input).unwrap().result
        };
        let here = three(&bodies);
        for m in MOVES {
            let moved: Vec<Body> = bodies.iter().map(|b| m.body(b)).collect();
            let there = three(&moved);
            for (k, (a, b)) in here.iter().zip(&there).enumerate() {
                let what = format!("{figure}, selection {k}, {m:?}");
                let volume = |r: &[Combined]| r.iter().map(|c| six(&c.body)).sum::<i128>();
                assert_eq!(volume(a), volume(b), "{what}");
                let want: BTreeSet<[i64; 3]> = a
                    .iter()
                    .flat_map(|c| corners(&c.body))
                    .map(|p| m.point(p))
                    .collect();
                let got: BTreeSet<[i64; 3]> = b.iter().flat_map(|c| corners(&c.body)).collect();
                assert_eq!(got, want, "{what}");
                both_pass(&what, b, &mut 0);
            }
        }
    }
}

/// A plane's `n · x + d = 0` as rationals.
fn plane_of(mesh: &ExactMesh, row: u32) -> ([RBig; 3], RBig) {
    let p = &mesh.parts().planes[row as usize];
    let q = |x: i128| RBig::from(IBig::from(x));
    (p.n().map(q), q(p.d()))
}

/// A mesh vertex's exact place: a grid point, or where its three planes meet (Cramer's rule).
fn place(mesh: &ExactMesh, v: u32) -> [RBig; 3] {
    match mesh.parts().verts[v as usize] {
        Vertex::Grid(p) => [p.x.get(), p.y.get(), p.z.get()].map(int),
        Vertex::Planes(rows) => {
            let [(a, da), (b, db), (c, dc)] = rows.map(|r| plane_of(mesh, r));
            let cross = |u: &[RBig; 3], w: &[RBig; 3]| {
                [
                    &u[1] * &w[2] - &u[2] * &w[1],
                    &u[2] * &w[0] - &u[0] * &w[2],
                    &u[0] * &w[1] - &u[1] * &w[0],
                ]
            };
            let dot = |u: &[RBig; 3], w: &[RBig; 3]| &u[0] * &w[0] + &u[1] * &w[1] + &u[2] * &w[2];
            let (bc, ca, ab) = (cross(&b, &c), cross(&c, &a), cross(&a, &b));
            let det = dot(&a, &bc);
            assert_ne!(det, RBig::ZERO, "a vertex of planes that do not meet");
            [0, 1, 2].map(|k| -(&da * &bc[k] + &db * &ca[k] + &dc * &ab[k]) / &det)
        }
    }
}

/// Six times the volume a result mesh bounds, exactly: each loop's fan of tetrahedra from the
/// origin, over its corners' exact places.
fn exact_six(mesh: &ExactMesh) -> RBig {
    let p = mesh.parts();
    let mut sum = RBig::ZERO;
    for pair in p.loop_start.windows(2) {
        let run = &p.he_vert[pair[0] as usize..pair[1] as usize];
        let at: Vec<[RBig; 3]> = run.iter().map(|v| place(mesh, *v)).collect();
        for k in 1..at.len().saturating_sub(1) {
            let (a, b, c) = (&at[0], &at[k], &at[k + 1]);
            sum += &a[0] * (&b[1] * &c[2] - &b[2] * &c[1])
                - &a[1] * (&b[0] * &c[2] - &b[2] * &c[0])
                + &a[2] * (&b[0] * &c[1] - &b[1] * &c[0]);
        }
    }
    sum
}

/// Every vertex's exact place.
fn places(mesh: &ExactMesh) -> BTreeSet<[RBig; 3]> {
    (0..u32::try_from(mesh.parts().verts.len()).unwrap())
        .map(|v| place(mesh, v))
        .collect()
}

/// The kernel's own six times the volume holds the exact one: equal while every corner is on the
/// grid, inside the certified interval once one is not.
fn holds(kernel: Volume, exact: &RBig) -> bool {
    match kernel {
        Volume::Exact(v) => {
            let limbs = v.to_le_limbs();
            let mut magnitude = IBig::ZERO;
            for limb in limbs.iter().rev() {
                magnitude = (magnitude << 64_usize) + IBig::from(*limb);
            }
            let value = if v.is_negative() {
                magnitude - (IBig::ONE << (64 * limbs.len()))
            } else {
                magnitude
            };
            RBig::from(value) == *exact
        }
        Volume::Certified(v) => {
            let lo = RBig::try_from(v.lo()).unwrap();
            let hi = RBig::try_from(v.hi()).unwrap();
            lo <= *exact && *exact <= hi
        }
    }
}

/// A model on the grid, transformed: its points moved and, for a mirror, its faces turned back so
/// they face out.
fn transformed(grid: &Grid, m: Move) -> Grid {
    let mut out = grid.clone();
    for p in &mut out.points {
        *p = m.point(*p);
    }
    if matches!(m, Move::Mirror) {
        for f in &mut out.faces {
            f.reverse();
        }
    }
    out
}

/// The vector a model's copy moves by: a third of its extent along each axis, and a step.
fn offset(grid: &Grid) -> [i64; 3] {
    [0, 1, 2].map(|k| {
        let lo = grid.points.iter().map(|p| p[k]).min().unwrap();
        let hi = grid.points.iter().map(|p| p[k]).max().unwrap();
        (hi - lo) / 3 + 1
    })
}

/// What a model gives: refused by the builder with its error, or the union of it alone and of it
/// with its moved copy.
enum Read {
    Refused(String),
    Built { alone: ExactMesh, pair: ExactMesh },
}

/// The model `grid`, built and united alone and with its copy moved by `by`, on `exec`.
fn unions(grid: &Grid, by: [i64; 3], exec: &Exec<'_>) -> Read {
    let mesh =
        |by: [i64; 3], name: u8| build(&mesh_polygons(grid, by, name).unwrap(), exec, &mut meter());
    let (a, b) = match (mesh([0; 3], 1), mesh(by, 2)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => return Read::Refused(format!("{e:?}")),
    };
    let step = OpPath::new(OpId::from_bytes([9; 16]));
    let tests = [WindingTest::unite()];
    let alone = boolean(&[&a], &tests, &step, exec, &mut meter()).unwrap();
    let pair = boolean(&[&a, &b], &tests, &step, exec, &mut meter()).unwrap();
    Read::Built {
        alone: alone.into_iter().next().unwrap().mesh,
        pair: pair.into_iter().next().unwrap().mesh,
    }
}

/// The most faces a model of the pull-request tier has: the seven larger ones (4,418 to 19,704
/// faces) run in the nightly tier, `the_large_models_unite_to_the_reference`.
const LARGE: usize = 1_500;

/// Every model of a tier, read onto the grid: those of at most [`LARGE`] faces, or the rest.
fn models(large: bool) -> Vec<(&'static str, Grid)> {
    MODELS
        .iter()
        .map(|(name, text)| (*name, obj::read(text).unwrap().to_grid().unwrap()))
        .filter(|(_, grid)| (grid.faces.len() > LARGE) == large)
        .collect()
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "the reference's rationals take minutes in a debug build; every release leg runs it"
)]
fn manifold_s_models_unite_alone_and_with_a_moved_copy_to_the_reference() {
    let refused = united_to_the_reference(false);
    assert_eq!(named(&refused), REFUSED);
}

#[test]
#[ignore = "the nightly tier: the seven models of more than 1,500 faces, which take minutes; \
            kernel-nightly.yml runs it"]
fn the_large_models_unite_to_the_reference() {
    let refused = united_to_the_reference(true);
    assert_eq!(named(&refused), REFUSED_LARGE);
}

/// Refusals as the tables write them.
fn named<'r>(refused: &'r [(&'static str, String)]) -> Vec<(&'static str, &'r str)> {
    refused
        .iter()
        .map(|(name, why)| (*name, why.as_str()))
        .collect()
}

/// Every model of a tier united alone and with its moved copy, each result's exact volume the
/// reference's and inside the kernel's own; the models the builder refused, with why.
fn united_to_the_reference(large: bool) -> Vec<(&'static str, String)> {
    let mut refused = Vec::new();
    for (name, grid) in models(large) {
        let by = offset(&grid);
        let (alone, pair) = match unions(&grid, by, &Exec::serial()) {
            Read::Refused(why) => {
                refused.push((name, why));
                continue;
            }
            Read::Built { alone, pair } => (alone, pair),
        };
        let a = mesh_solid(&grid, [0; 3]).unwrap();
        let b = mesh_solid(&grid, by).unwrap();
        for (what, mesh, want) in [
            (
                "alone",
                &alone,
                six_volume_where(&[&a], |w| w[0] > 0).unwrap(),
            ),
            (
                "with a moved copy",
                &pair,
                six_volume_where(&[&a, &b], |w| w[0] > 0 || w[1] > 0).unwrap(),
            ),
        ] {
            let got = exact_six(mesh);
            assert_eq!(got, want, "{name} {what}: kernel against the reference");
            assert!(
                holds(mesh.volume(&mut meter()).unwrap(), &got),
                "{name} {what}: the kernel's volume does not hold its exact one"
            );
        }
    }
    refused
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "three transforms of every model take minutes in a debug build; every release leg \
              runs it"
)]
fn manifold_s_models_commute_with_exact_transforms() {
    for (name, grid) in models(false) {
        let by = offset(&grid);
        let Read::Built { pair, .. } = unions(&grid, by, &Exec::serial()) else {
            continue;
        };
        for m in MOVES {
            let moved = transformed(&grid, m);
            // The copy moves by the vector moved by the transform's linear part.
            let at = m.point(by);
            let origin = m.point([0; 3]);
            let copy = [0, 1, 2].map(|k| at[k] - origin[k]);
            let Read::Built { pair: there, .. } = unions(&moved, copy, &Exec::serial()) else {
                panic!("{name}: {m:?} refused");
            };
            assert_eq!(exact_six(&there), exact_six(&pair), "{name} {m:?}: volume");
            let want: BTreeSet<[RBig; 3]> = places(&pair).into_iter().map(|p| m.exact(p)).collect();
            assert_eq!(places(&there), want, "{name} {m:?}: corners");
        }
    }
}

fn put(total: &mut Sha256, bytes: &[u8]) {
    total.write(&(bytes.len() as u64).to_le_bytes());
    total.write(bytes);
}

fn facings(total: &mut Sha256, items: &[Facing]) {
    put(total, &(items.len() as u64).to_le_bytes());
    for f in items {
        put(total, &f.row.to_le_bytes());
        put(total, &[u8::from(f.flipped)]);
    }
}

fn words(total: &mut Sha256, items: &[u32]) {
    put(total, &(items.len() as u64).to_le_bytes());
    for i in items {
        put(total, &i.to_le_bytes());
    }
}

/// A result mesh's arrays and runs, in canonical order.
fn write_mesh(total: &mut Sha256, m: &ExactMesh) {
    let p = m.parts();
    put(total, &(p.planes.len() as u64).to_le_bytes());
    for plane in &p.planes {
        for c in plane.n() {
            put(total, &c.to_le_bytes());
        }
        put(total, &plane.d().to_le_bytes());
    }
    put(total, &(p.verts.len() as u64).to_le_bytes());
    for v in &p.verts {
        match v {
            Vertex::Grid(c) => {
                for x in [c.x.get(), c.y.get(), c.z.get()] {
                    put(total, &x.to_le_bytes());
                }
            }
            Vertex::Planes(rows) => {
                for r in rows {
                    put(total, &r.to_le_bytes());
                }
            }
        }
    }
    facings(total, &p.face_plane);
    words(total, &p.face_loops);
    words(total, &p.loop_start);
    words(total, &p.he_vert);
    facings(total, &p.he_plane);
    words(total, &p.he_pair);
    put(total, format!("{:?}", p.runs).as_bytes());
}

/// The corpus on `exec`: every figure's every selection, each body's records, and every model's
/// unions, each mesh's arrays; with how many results went in.
fn corpus(exec: &Exec<'_>) -> (Sha256, usize) {
    let mut total = Sha256::new();
    let mut results = 0;
    for (_, blocks) in figures() {
        for bodies in select_all(&blocks, exec) {
            put(&mut total, &(bodies.len() as u64).to_le_bytes());
            for one in &bodies {
                // A body the format refuses goes in by its refusal, as a refused model does:
                // `every_figure_result_is_written_in_the_native_format` fails on it.
                match write_body(&one.body, &mut meter()) {
                    Ok(stored) => {
                        for r in stored.chunks.iter().chain([&stored.index]) {
                            put(&mut total, &r.encode(&mut meter()).unwrap());
                        }
                    }
                    Err(why) => put(&mut total, format!("{why:?}").as_bytes()),
                }
                results += 1;
            }
        }
    }
    for (name, grid) in models(false) {
        put(&mut total, name.as_bytes());
        match unions(&grid, offset(&grid), exec) {
            Read::Refused(why) => put(&mut total, why.as_bytes()),
            Read::Built { alone, pair } => {
                write_mesh(&mut total, &alone);
                write_mesh(&mut total, &pair);
                results += 2;
            }
        }
    }
    (total, results)
}

#[test]
fn the_polyhedral_corpus_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let (total, results) = corpus(&Exec::serial());
        let digest = total.finish();
        (digest, (digest, results))
    });
    let (digest, results) = checked.value;
    println!(
        "polyhedral corpus: {results} results, golden hash {}",
        hex(digest)
    );
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target orders a body's records or a mesh's \
             arrays differently (compare this log's digest with another target's), or an \
             operation changed what it returns in place"
        )
    });
}

/// Chapter 21's thread-count test: the same digest on every pool of the matrix's thread counts.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "five runs of the corpus; every release leg runs it"
)]
#[cfg_attr(
    all(target_family = "wasm", not(target_feature = "atomics")),
    ignore = "WebAssembly without the atomics feature (wasm32-wasip1) has no threads: \
              std::thread::scope cannot spawn"
)]
fn the_polyhedral_corpus_is_the_same_on_every_thread_count() {
    let one = hex(corpus(&Exec::serial()).0.finish());
    for threads in golden::THREAD_COUNTS {
        let pool = Threads(threads);
        let got = hex(corpus(&Exec::with_pool(&pool)).0.finish());
        assert_eq!(got, one, "{threads} threads");
    }
}
