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
//! Arguments: a side (23, 71 or 224; all three by default), then text a run's line must hold to
//! run, such as "in a strip, as solids, 1 thread". Each timed run is the call `timed`, so a
//! profiler can count it alone: `valgrind --tool=callgrind --collect-atstart=no
//! --toggle-collect='boolean::timed*'`. With that text, the operands are made on one thread, and
//! a pair it does not name is not made.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
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

fn main() {
    let rayon = Rayon::new(NonZeroUsize::new(8).unwrap()).unwrap();
    let step = OpPath::new(OpId::from_bytes([7; 16]));
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
            println!(
                "{run}: soup of {kept} pieces in {staged:?} (3 x EMBER: 4.8ms); {faces} faces in                  {time:?}, so merge and build {:?} (budget {budget:?})",
                time.saturating_sub(staged)
            );
        }
    }
}
