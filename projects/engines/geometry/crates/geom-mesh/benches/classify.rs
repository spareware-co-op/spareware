//! How fast points are located, by the standard library's clock (piece G147): the done-when's
//! million queries on 100,000 faces in a second on 8 threads, on a closed height field, whose rays
//! each cross a top and a bottom. Prints one line per run; the gate is the count benchmark in
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

use geom_mesh::classify::{Classifier, Located};
use geom_mesh::exact::build::{Corner, Polygons, Source, build};
use geom_mesh::exact::{PropTable, Role};
use geom_num::Exactness;
use geom_num::grid::P3;
use geom_op::NameId;
use geom_pool::native::Rayon;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::{Seed, SplitMix64};

/// A closed solid of about 100,000 faces: a field of triangles above a flat base.
fn field(rng: &mut SplitMix64) -> Polygons {
    let side = 160_u32;
    let height = NonZeroU64::new(1 << 12).unwrap();
    let mut vertices = Vec::new();
    for y in 0..side {
        for x in 0..side {
            let z = 4096 + rng.below(height) as i64;
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
    let mut rng = SplitMix64::new(Seed(0x147));
    let input = field(&mut rng);
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let mesh = build(&input, &Exec::serial(), &mut meter).unwrap();
    let start = Instant::now();
    let classifier = Classifier::new(&mesh, &Exec::serial(), &mut meter).unwrap();
    println!(
        "{} faces: classifier in {:?}",
        mesh.faces(),
        start.elapsed()
    );
    let reach = NonZeroU64::new(159 * 4096).unwrap();
    let tall = NonZeroU64::new(1 << 13).unwrap();
    let points: Vec<P3> = (0..1_000_000)
        .map(|_| {
            P3::new(
                rng.below(reach) as i64,
                rng.below(reach) as i64,
                rng.below(tall) as i64 + 1,
            )
            .unwrap()
        })
        .collect();
    let rayon = Rayon::new(NonZeroUsize::new(8).unwrap()).unwrap();
    for (name, exec) in [
        ("1 thread", Exec::serial()),
        ("8 threads", Exec::with_pool(&rayon)),
    ] {
        let start = Instant::now();
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let found = classifier.locate_all(&points, &exec, &mut meter).unwrap();
        let time = start.elapsed();
        let inside = found.iter().filter(|l| **l == Located::Winding(1)).count();
        println!(
            "{name}: {} points in {time:?}, {inside} inside",
            points.len()
        );
    }
}
