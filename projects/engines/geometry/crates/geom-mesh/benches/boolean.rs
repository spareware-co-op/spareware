//! How fast meshes combine, by the standard library's clock (piece G148): two closed height fields
//! of about 2,000, 20,000 and 200,000 faces, offset so that their tops cross all over. EMBER's
//! geometric mean over 1,000 Thingi10K pairs of 1,000 to 100,000 faces was 1.6 ms; those models are
//! not stored here, so this prints what these pairs take. Prints one line per run; the gate is the
//! count benchmark in `geom-bench`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::print_stdout,
    reason = "a benchmark program; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::{NonZeroU64, NonZeroUsize};
use std::time::Instant;

use geom_mesh::boolean::boolean;
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

fn main() {
    let rayon = Rayon::new(NonZeroUsize::new(8).unwrap()).unwrap();
    let step = OpPath::new(OpId::from_bytes([7; 16]));
    for side in [71, 224] {
        let mut rng = SplitMix64::new(Seed(0x148));
        let a = mesh(&field(side, 0, 1, &mut rng), &Exec::with_pool(&rayon));
        let b = mesh(&field(side, 2048, 2, &mut rng), &Exec::with_pool(&rayon));
        for (name, exec) in [
            ("1 thread", Exec::serial()),
            ("8 threads", Exec::with_pool(&rayon)),
        ] {
            let start = Instant::now();
            let mut meter = WorkMeter::new(Budget::UNLIMITED);
            let done =
                boolean(&[&a, &b], &[WindingTest::unite()], &step, &exec, &mut meter).unwrap();
            let time = start.elapsed();
            println!(
                "{} + {} faces, {name}: {} faces in {time:?}",
                a.faces(),
                b.faces(),
                done[0].mesh.faces()
            );
        }
    }
}
