//! How fast meshes are sliced, by the standard library's clock (piece G149): the done-when's million
//! faces into 1,000 layers in a second on 8 threads, on a closed height field. Prints one line per
//! run; the gate is the count benchmark in `geom-bench`.

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
use geom_mesh::exact::{PropTable, Role};
use geom_mesh::slice::slice;
use geom_num::Exactness;
use geom_num::grid::P3;
use geom_num::plane::Plane;
use geom_op::NameId;
use geom_pool::native::Rayon;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::{Seed, SplitMix64};

/// A closed solid of about 1,000,000 faces: a rough field of triangles above a flat base.
fn field(rng: &mut SplitMix64) -> Polygons {
    let side = 501_u32;
    let rough = NonZeroU64::new(1 << 14).unwrap();
    let mut vertices = Vec::new();
    for y in 0..side {
        for x in 0..side {
            let hill = i64::from(x.min(side - x) + y.min(side - y)) * 64;
            let z = 4096 + hill + rng.below(rough) as i64;
            let at = P3::new(i64::from(x) * 4096, i64::from(y) * 4096, z).unwrap();
            vertices.push(Corner::Grid(at));
        }
    }
    let flat = |x: u32, y: u32| {
        Corner::Grid(P3::new(i64::from(x) * 4096, i64::from(y) * 4096, 0).unwrap())
    };
    let low = side * side;
    for y in 0..side {
        for x in 0..side {
            vertices.push(flat(x, y));
        }
    }
    let at = |x: u32, y: u32| y * side + x;
    let mut corners = Vec::new();
    let mut loop_start = vec![0_u32];
    let mut face = |vs: &[u32]| {
        corners.extend_from_slice(vs);
        loop_start.push(corners.len() as u32);
    };
    for y in 0..side - 1 {
        for x in 0..side - 1 {
            let (a, b, c, d) = (at(x, y), at(x + 1, y), at(x + 1, y + 1), at(x, y + 1));
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
    }
}

fn main() {
    let mut rng = SplitMix64::new(Seed(0x149));
    let input = field(&mut rng);
    let rayon = Rayon::new(NonZeroUsize::new(8).unwrap()).unwrap();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let mesh = build(&input, &Exec::with_pool(&rayon), &mut meter).unwrap();
    let planes: Vec<Plane> = (0..1000)
        .map(|k| Plane::new([0, 0, 1], -i128::from(4097 + k * 48)).unwrap())
        .collect();
    for (name, exec) in [
        ("1 thread", Exec::serial()),
        ("8 threads", Exec::with_pool(&rayon)),
    ] {
        let start = Instant::now();
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let layers = slice(&mesh, &planes, &exec, &mut meter).unwrap();
        let time = start.elapsed();
        let rings: usize = layers.iter().map(Vec::len).sum();
        let cuts: usize = layers.iter().flatten().map(|c| c.cuts().len()).sum();
        println!(
            "{name}: {} faces into {} layers in {time:?}, {rings} rings of {cuts} points",
            mesh.faces(),
            planes.len()
        );
    }
}
