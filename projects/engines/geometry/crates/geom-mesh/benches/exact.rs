//! How long an exact mesh takes to build and check, by the standard library's clock (piece G146):
//! a million triangles, the done-when's half second, on a height field whose every face has its own
//! plane, the hard case for the pool. Prints one line per run; the gate is the count benchmark in
//! `geom-bench`.

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

use geom_mesh::exact::build::{Corner, Polygons, Source, build};
use geom_mesh::exact::{ExactMesh, PropTable, Role};
use geom_num::Exactness;
use geom_num::grid::P3;
use geom_op::NameId;
use geom_pool::native::Rayon;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::{Seed, SplitMix64};

fn main() {
    let mut rng = SplitMix64::new(Seed(0x146));
    let side = 708_i64;
    let height = NonZeroU64::new(1 << 12).unwrap();
    let vertices: Vec<Corner> = (0..side * side)
        .map(|i| {
            let (x, y) = ((i % side) * 4096, (i / side) * 4096);
            Corner::Grid(P3::new(x, y, rng.below(height) as i64).unwrap())
        })
        .collect();
    let mut corners = Vec::new();
    for y in 0..side as u32 - 1 {
        for x in 0..side as u32 - 1 {
            let at = y * side as u32 + x;
            let (a, b, c, d) = (at, at + 1, at + side as u32 + 1, at + side as u32);
            corners.extend([a, b, c, a, c, d]);
        }
    }
    let faces = corners.len() / 3;
    let input = Polygons {
        vertices,
        corners,
        loop_start: (0..=faces as u32).map(|f| f * 3).collect(),
        face_loops: (0..=faces as u32).collect(),
        face_run: vec![0; faces],
        runs: vec![Source {
            name: NameId::from_bytes([1; 16]),
            role: Role::Face,
            flags: 0,
        }],
        face_plane: Vec::new(),
        edge_plane: Vec::new(),
        corner_prop: Vec::new(),
        props: PropTable::default(),
        face_dev: Vec::new(),
        class: Exactness::Exact,
    };
    let threads = NonZeroUsize::new(8).unwrap();
    let rayon = Rayon::new(threads).unwrap();
    for (name, exec) in [
        ("1 thread", Exec::serial()),
        ("8 threads", Exec::with_pool(&rayon)),
    ] {
        let start = Instant::now();
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let mesh = build(&input, &exec, &mut meter).unwrap();
        let built = start.elapsed();
        let start = Instant::now();
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let mesh = ExactMesh::new(mesh.into_parts(), &exec, &mut meter).unwrap();
        println!(
            "{name}: {faces} faces, {} planes: built and checked in {built:?}, checked again in {:?}",
            mesh.parts().planes.len(),
            start.elapsed()
        );
    }
}
