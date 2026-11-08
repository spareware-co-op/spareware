//! How fast meshes combine, by the standard library's clock (piece G148): two closed height fields
//! of about 2,000, 20,000 and 200,000 faces, offset half a cell so that their tops cross all over,
//! and offset four fifths of their width so that they overlap in a strip, as a pair of models
//! placed side by side mostly do. EMBER's geometric mean over 1,000 Thingi10K pairs of 1,000 to
//! 100,000 faces was 1.6 ms; those models are not stored here, so this prints what these pairs
//! take, with the operands as built and as solids (each first united alone, which proves it, as
//! 11.3 step 4 has every checked operand be). Prints one line per run; the gate is the count
//! benchmark in `geom-bench`.
//!
//! Each run is timed twice, as G148's stage budgets ask (`projects/kit/notes/G148-stage-budgets.md`):
//! the polygon soup alone (`soup`), the stage EMBER's 1.6 ms times, against 3 times that, 4.8 ms;
//! and the whole boolean, whose merge and build, the difference, are held to 0.5 µs for each kept
//! piece and each result face. The whole time is printed beside both.
//!
//! A second mode, `shapes`, times pairs that cut as real models do, in a curve, so most faces are
//! untouched: two spheres, a sphere and a torus, two tori, and a cube against a rotated cube, each
//! of about 1,000, 10,000 and 100,000 faces a side, made by a fixed generator (no random numbers).
//! Thingi10K's own models cannot be read yet (the readers are not built), so this is the stand-in
//! for EMBER's 1.6 ms; `shapes` takes the same filter text, such as `shapes sphere`, and `small` and `one` (only the one-thread runs) after it, `small` for radius 2^13 steps, where every plane's coefficients fit machine words (otherwise 2^24).
//!
//! Arguments: a side (23, 71 or 224; all three by default), then text a run's line must hold to
//! run, such as "in a strip, as solids, 1 thread". Each timed run is the call `timed`, so a
//! profiler can count it alone: `valgrind --tool=callgrind --collect-atstart=no
//! --toggle-collect='boolean::timed*'`. With that text, the operands are made on one thread, and
//! a pair it does not name is not made.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::doc_markdown,
    clippy::many_single_char_names,
    clippy::print_stdout,
    reason = "a benchmark program; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::{NonZeroU64, NonZeroUsize};
use std::time::{Duration, Instant};

use geom_mesh::boolean::{boolean, soup};
use geom_mesh::exact::ExactMesh;
use geom_mesh::exact::build::{Corner, Polygons, Source, build};
use geom_mesh::exact::{PropTable, Role};
use geom_num::Exactness;
use geom_num::grid::P3;
use geom_op::{NameId, OpId, OpPath};
use geom_poly::winding::WindingTest;
use geom_pool::native::Rayon;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::{Seed, SplitMix64};

/// A closed solid of about `4 · side²` faces: rough ground over a flat base, moved along x by `x`.
fn field(side: u32, x: i64, name: u8, rng: &mut SplitMix64) -> Polygons {
    let rough = NonZeroU64::new(1 << 12).unwrap();
    let mut vertices = Vec::new();
    for y in 0..side {
        for i in 0..side {
            let z = 4096 + rng.below(rough) as i64;
            let at = P3::new(i64::from(i) * 4096 + x, i64::from(y) * 4096, z).unwrap();
            vertices.push(Corner::Grid(at));
        }
    }
    let flat = |i: u32, y: u32| {
        Corner::Grid(P3::new(i64::from(i) * 4096 + x, i64::from(y) * 4096, 0).unwrap())
    };
    let low = side * side;
    for y in 0..side {
        for i in 0..side {
            vertices.push(flat(i, y));
        }
    }
    let at = |i: u32, y: u32| y * side + i;
    let mut corners = Vec::new();
    let mut loop_start = vec![0_u32];
    let mut face = |vs: &[u32]| {
        corners.extend_from_slice(vs);
        loop_start.push(corners.len() as u32);
    };
    for y in 0..side - 1 {
        for i in 0..side - 1 {
            let (a, b, c, d) = (at(i, y), at(i + 1, y), at(i + 1, y + 1), at(i, y + 1));
            face(&[a, b, c]);
            face(&[a, c, d]);
            face(&[low + a, low + c, low + b]);
            face(&[low + a, low + d, low + c]);
        }
    }
    for i in 0..side - 1 {
        let walls = [
            (at(i, 0), at(i + 1, 0)),
            (at(side - 1, i), at(side - 1, i + 1)),
            (at(i + 1, side - 1), at(i, side - 1)),
            (at(0, i + 1), at(0, i)),
        ];
        for (a, b) in walls {
            face(&[low + a, low + b, b, a]);
        }
    }
    let faces = loop_start.len() as u32 - 1;
    Polygons {
        vertices,
        corners,
        loop_start,
        face_loops: (0..=faces).collect(),
        face_run: vec![0; faces as usize],
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
    }
}

fn mesh(input: &Polygons, exec: &Exec<'_>) -> ExactMesh {
    build(input, exec, &mut WorkMeter::new(Budget::UNLIMITED)).unwrap()
}

/// One timed run: the union of `a` and `b`, and the faces it has.
#[inline(never)]
fn timed(a: &ExactMesh, b: &ExactMesh, step: &OpPath, exec: &Exec<'_>) -> usize {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let done = boolean(&[a, b], &[WindingTest::unite()], step, exec, &mut meter).unwrap();
    done[0].mesh.faces()
}

/// The union's polygon soup alone, and how many pieces it keeps.
#[inline(never)]
fn soup_only(a: &ExactMesh, b: &ExactMesh, exec: &Exec<'_>) -> usize {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    soup(&[a, b], &[WindingTest::unite()], exec, &mut meter).unwrap()[0].len()
}

/// A closed triangle mesh as polygons: `points` on the grid and `tris` of indices into them,
/// turned about if need be so that they face out of the solid they close.
fn triangles(points: &[[i64; 3]], tris: &[[u32; 3]], name: u8) -> Polygons {
    // Six times the volume by the divergence theorem: negative if the faces point in.
    let at = |i: u32| points[i as usize].map(i128::from);
    let mut volume = 0_i128;
    for t in tris {
        let (a, b, c) = (at(t[0]), at(t[1]), at(t[2]));
        volume += a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]);
    }
    let vertices = points
        .iter()
        .map(|p| Corner::Grid(P3::new(p[0], p[1], p[2]).unwrap()))
        .collect();
    let mut corners = Vec::new();
    let mut loop_start = vec![0_u32];
    for t in tris {
        if volume < 0 {
            corners.extend_from_slice(&[t[0], t[2], t[1]]);
        } else {
            corners.extend_from_slice(t);
        }
        loop_start.push(corners.len() as u32);
    }
    let faces = loop_start.len() as u32 - 1;
    Polygons {
        vertices,
        corners,
        loop_start,
        face_loops: (0..=faces).collect(),
        face_run: vec![0; faces as usize],
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
    }
}

/// A surface of rows of `around` points, wrapped round in both directions (a torus) or closed at
/// two poles (a sphere): `at(row, column)` gives each point, and `rows` rows are made.
fn wrapped(
    rows: u32,
    around: u32,
    poles: Option<[[i64; 3]; 2]>,
    at: &dyn Fn(u32, u32) -> [i64; 3],
) -> (Vec<[i64; 3]>, Vec<[u32; 3]>) {
    let mut points = Vec::new();
    for r in 0..rows {
        for c in 0..around {
            points.push(at(r, c));
        }
    }
    let index = |r: u32, c: u32| (r % rows) * around + c % around;
    let mut tris = Vec::new();
    let bands = if poles.is_some() { rows - 1 } else { rows };
    if let Some(pole) = poles {
        let (north, south) = (points.len() as u32, points.len() as u32 + 1);
        points.extend(pole);
        for c in 0..around {
            tris.push([north, index(0, c), index(0, c + 1)]);
            tris.push([south, index(rows - 1, c + 1), index(rows - 1, c)]);
        }
    }
    for r in 0..bands {
        for c in 0..around {
            let (a, b, d, e) = (
                index(r, c),
                index(r, c + 1),
                index(r + 1, c),
                index(r + 1, c + 1),
            );
            tris.push([a, d, b]);
            tris.push([b, d, e]);
        }
    }
    (points, tris)
}

/// A sphere of radius `radius` about `center`: `rings - 1` rows of `around` points, and
/// `2 · around · (rings - 1)` faces.
fn sphere(rings: u32, around: u32, radius: f64, center: [i64; 3], name: u8) -> Polygons {
    let at = |r: u32, c: u32| {
        let (theta, phi) = (
            std::f64::consts::PI * f64::from(r + 1) / f64::from(rings),
            std::f64::consts::TAU * f64::from(c) / f64::from(around),
        );
        [
            center[0] + (radius * theta.sin() * phi.cos()).round() as i64,
            center[1] + (radius * theta.sin() * phi.sin()).round() as i64,
            center[2] + (radius * theta.cos()).round() as i64,
        ]
    };
    let top = radius.round() as i64;
    let pole = [
        [center[0], center[1], center[2] + top],
        [center[0], center[1], center[2] - top],
    ];
    let (points, tris) = wrapped(rings - 1, around, Some(pole), &at);
    triangles(&points, &tris, name)
}

/// A torus of major radius `major` and minor radius `minor` about `center`, of
/// `2 · rings · around` faces.
fn torus(
    rings: u32,
    around: u32,
    (major, minor): (f64, f64),
    center: [i64; 3],
    name: u8,
) -> Polygons {
    let at = |r: u32, c: u32| {
        let (v, u) = (
            std::f64::consts::TAU * f64::from(r) / f64::from(rings),
            std::f64::consts::TAU * f64::from(c) / f64::from(around),
        );
        let ring = major + minor * v.cos();
        [
            center[0] + (ring * u.cos()).round() as i64,
            center[1] + (ring * u.sin()).round() as i64,
            center[2] + (minor * v.sin()).round() as i64,
        ]
    };
    let (points, tris) = wrapped(rings, around, None, &at);
    triangles(&points, &tris, name)
}

/// A cube of half-width `half` about `center`, each face `k` by `k` squares of two triangles; when
/// `turned`, rotated about z by the angle with cosine 3/5 and then about x by the one with cosine
/// 5/13, each point rounded to the grid after. `12 · k²` faces.
fn cube(k: u32, half: i64, center: [i64; 3], turned: bool, name: u8) -> Polygons {
    let coordinate = |i: u32| -half + (2 * half * i64::from(i) + i64::from(k) / 2) / i64::from(k);
    let mut ids = std::collections::BTreeMap::new();
    let mut points: Vec<[i64; 3]> = Vec::new();
    let mut tris = Vec::new();
    let mut point = |p: [u32; 3]| -> u32 {
        *ids.entry(p).or_insert_with(|| {
            let mut v = p.map(coordinate).map(|x| x as f64);
            if turned {
                v = [
                    (3.0 * v[0] - 4.0 * v[1]) / 5.0,
                    (4.0 * v[0] + 3.0 * v[1]) / 5.0,
                    v[2],
                ];
                v = [
                    v[0],
                    (5.0 * v[1] - 12.0 * v[2]) / 13.0,
                    (12.0 * v[1] + 5.0 * v[2]) / 13.0,
                ];
            }
            points.push([
                center[0] + v[0].round() as i64,
                center[1] + v[1].round() as i64,
                center[2] + v[2].round() as i64,
            ]);
            points.len() as u32 - 1
        })
    };
    for axis in 0..3 {
        for end in [0, k] {
            for i in 0..k {
                for j in 0..k {
                    let corner = |a: u32, b: u32| {
                        let mut p = [0; 3];
                        p[axis] = end;
                        p[(axis + 1) % 3] = a;
                        p[(axis + 2) % 3] = b;
                        p
                    };
                    let (a, b, c, d) = (
                        point(corner(i, j)),
                        point(corner(i + 1, j)),
                        point(corner(i + 1, j + 1)),
                        point(corner(i, j + 1)),
                    );
                    // Facing out: along the axis at its far end, against it at its near end.
                    if end == k {
                        tris.push([a, b, c]);
                        tris.push([a, c, d]);
                    } else {
                        tris.push([a, c, b]);
                        tris.push([a, d, c]);
                    }
                }
            }
        }
    }
    triangles(&points, &tris, name)
}

/// The pairs of the `shapes` mode at a size class (0, 1 or 2: about 1,000, 10,000 and 100,000
/// faces a side).
fn shape_pair(kind: &str, size: usize, small: bool) -> (Polygons, Polygons) {
    // Radius 2^24 grid steps, or 2^13 where every plane's coefficients fit machine words.
    let s = 1_i64 << if small { 13 } else { 24 };
    let sf = s as f64;
    // Rings and points round a sphere, rings and points round a torus, and k of a cube.
    let round = [(17, 32), (51, 100), (159, 316)][size];
    let ring = [(16, 32), (50, 100), (158, 316)][size];
    let k = [9, 29, 91][size];
    match kind {
        "two spheres" => (
            sphere(round.0, round.1, sf, [0; 3], 1),
            sphere(round.0, round.1, sf, [s / 2, 0, 0], 2),
        ),
        "a sphere and a torus" => (
            sphere(round.0, round.1, sf, [0; 3], 1),
            torus(ring.0, ring.1, (sf, sf * 0.35), [s / 5, 0, 0], 2),
        ),
        "two tori" => (
            torus(ring.0, ring.1, (sf, sf * 0.35), [0; 3], 1),
            torus(ring.0, ring.1, (sf, sf * 0.35), [s, 0, 0], 2),
        ),
        _ => (cube(k, s, [0; 3], false, 1), cube(k, s, [0; 3], true, 2)),
    }
}

/// The `shapes` mode: each pair at each size, as built (the models are closed), on 1 and 8
/// threads, with the soup alone and the whole boolean.
fn shapes(rayon: &Rayon, step: &OpPath, only: &str, (small, one_thread): (bool, bool)) {
    for kind in [
        "two spheres",
        "a sphere and a torus",
        "two tori",
        "a cube and a turned cube",
    ] {
        for size in 0..3 {
            let label = format!(
                "{kind}, size class {size}{}",
                if small { ", small" } else { "" }
            );
            if !only.is_empty() && !label.contains(only) {
                continue;
            }
            let (pa, pb) = shape_pair(kind, size, small);
            let setup = Exec::with_pool(rayon);
            let (a, b) = (mesh(&pa, &setup), mesh(&pb, &setup));
            for (name, exec) in [
                ("1 thread", Exec::serial()),
                ("8 threads", Exec::with_pool(rayon)),
            ]
            .into_iter()
            .take(if one_thread { 1 } else { 2 })
            {
                let start = Instant::now();
                let kept = soup_only(&a, &b, &exec);
                let staged = start.elapsed();
                let start = Instant::now();
                let faces = timed(&a, &b, step, &exec);
                let time = start.elapsed();
                println!(
                    "{} + {} faces, {label}, {name}: soup of {kept} pieces in {staged:?} (3 x EMBER: 4.8ms); {faces} faces in {time:?}",
                    a.faces(),
                    b.faces()
                );
            }
        }
    }
}

fn main() {
    let rayon = Rayon::new(NonZeroUsize::new(8).unwrap()).unwrap();
    let step = OpPath::new(OpId::from_bytes([7; 16]));
    if std::env::args().nth(1).as_deref() == Some("shapes") {
        let filter = std::env::args().nth(2).unwrap_or_default();
        let options = std::env::args().skip(3).collect::<Vec<_>>();
        let has = |word: &str| options.iter().any(|o| o == word);
        shapes(&rayon, &step, &filter, (has("small"), has("one")));
        return;
    }
    let sides: Vec<u32> = match std::env::args().nth(1).and_then(|a| a.parse().ok()) {
        Some(side) => vec![side],
        None => vec![23, 71, 224],
    };
    let only = std::env::args().nth(2).unwrap_or_default();
    for (side, (pair, offset)) in sides
        .iter()
        .flat_map(|&side| [("crossing", 2048), ("in a strip", 0)].map(|p| (side, p)))
    {
        let offset = if offset == 0 {
            i64::from(side) * 4096 * 4 / 5
        } else {
            offset
        };
        // A filter that names the other pair skips this one's making.
        if ["crossing", "in a strip"].iter().any(|p| only.contains(p)) && !only.contains(pair) {
            continue;
        }
        // With a filter, the operands are made on one thread: a pool's threads are counted from
        // their start, so a profiler counting `timed` alone would count them too.
        let setup = if only.is_empty() {
            Exec::with_pool(&rayon)
        } else {
            Exec::serial()
        };
        let mut rng = SplitMix64::new(Seed(0x148));
        let a = mesh(&field(side, 0, 1, &mut rng), &setup);
        let b = mesh(&field(side, offset, 2, &mut rng), &setup);
        let alone = |m: &ExactMesh| {
            let mut meter = WorkMeter::new(Budget::UNLIMITED);
            boolean(&[m], &[WindingTest::unite()], &step, &setup, &mut meter)
                .unwrap()
                .remove(0)
                .mesh
        };
        let (solid_a, solid_b) = (alone(&a), alone(&b));
        for ((name, exec), (form, a, b)) in [
            ("1 thread", Exec::serial()),
            ("8 threads", Exec::with_pool(&rayon)),
        ]
        .into_iter()
        .flat_map(|run| {
            [
                (run, ("as built", &a, &b)),
                (run, ("as solids", &solid_a, &solid_b)),
            ]
        }) {
            let run = format!("{} + {} faces {pair}, {form}, {name}", a.faces(), b.faces());
            if !run.contains(&only) {
                continue;
            }
            let start = Instant::now();
            let kept = soup_only(a, b, &exec);
            let staged = start.elapsed();
            let start = Instant::now();
            let faces = timed(a, b, &step, &exec);
            let time = start.elapsed();
            let budget = Duration::from_nanos(500 * (kept + faces) as u64);
            let rest = time.saturating_sub(staged);
            println!(
                "{run}: soup of {kept} pieces in {staged:?} (3 x EMBER: 4.8ms); {faces} faces in {time:?}"
            );
            println!("    so merge and build {rest:?} (budget {budget:?})");
        }
    }
}
