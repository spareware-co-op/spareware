//! How long reconstruction by winding number takes, by the standard library's clock (piece G299):
//! a cube whose six faces are each a grid of quads with a quad missing from the bottom, and a pile
//! of overlapping boxes. Prints one line per run; the gate is the count benchmark in `geom-bench`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::print_stdout,
    reason = "a benchmark program; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroU64;
use std::time::Instant;

use geom_mesh::exact::Role;
use geom_mesh::exact::build::Source;
use geom_mesh::exact::raw::RawMesh;
use geom_mesh::repair::{Reconstruct, reconstruct};
use geom_num::grid::P3;
use geom_op::NameId;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::{Seed, SplitMix64};

fn raw(points: &[[i64; 3]], faces: &[Vec<u32>]) -> RawMesh {
    let vertices = points
        .iter()
        .map(|&[x, y, z]| P3::new(x, y, z).unwrap())
        .collect();
    let mut corners = Vec::new();
    let mut start = vec![0];
    for f in faces {
        corners.extend_from_slice(f);
        start.push(corners.len() as u32);
    }
    RawMesh::new(vertices, corners, start).unwrap()
}

/// A cube of side `n * cell` whose six faces are each `n` by `n` quads, outward, less the bottom's
/// middle quad.
fn gridded_cube(n: i64, cell: i64) -> RawMesh {
    let side = n * cell;
    let mut points: Vec<[i64; 3]> = Vec::new();
    let mut faces: Vec<Vec<u32>> = Vec::new();
    // For each axis a and end e, the face on the plane x_a = e * side, with u and v the other two
    // axes in the order that turns the normal outward.
    for a in 0..3 {
        for e in 0..2 {
            let (u, v) = if e == 1 {
                ((a + 1) % 3, (a + 2) % 3)
            } else {
                ((a + 2) % 3, (a + 1) % 3)
            };
            for i in 0..n {
                for j in 0..n {
                    if a == 2 && e == 0 && i == n / 2 && j == n / 2 {
                        continue;
                    }
                    let mut quad = Vec::new();
                    for (di, dj) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
                        let mut p = [0_i64; 3];
                        p[a] = e * side;
                        p[u] = (i + di) * cell;
                        p[v] = (j + dj) * cell;
                        quad.push(points.len() as u32);
                        points.push(p);
                    }
                    faces.push(quad);
                }
            }
        }
    }
    raw(&points, &faces)
}

/// `count` boxes on a 6 by 6 by 6 lattice of cells, 1 to 3 cells a side.
fn pile(count: usize, cell: i64) -> RawMesh {
    let mut rng = SplitMix64::new(Seed(0x299));
    let mut below = |n: u64| rng.below(NonZeroU64::new(n).unwrap()) as i64;
    let (mut points, mut faces) = (Vec::new(), Vec::new());
    for _ in 0..count {
        let low = [0; 3].map(|_| below(4));
        let size = [0; 3].map(|_| 1 + below(3));
        let base = points.len() as u32;
        for i in 0..8_usize {
            points.push(
                [0, 1, 2].map(|a| (low[a] + if i >> a & 1 == 1 { size[a] } else { 0 }) * cell),
            );
        }
        for side in [
            [0, 4, 6, 2],
            [1, 3, 7, 5],
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [0, 2, 3, 1],
            [4, 5, 7, 6],
        ] {
            faces.push(side.map(|v| base + v).to_vec());
        }
    }
    raw(&points, &faces)
}

fn time(name: &str, mesh: &RawMesh) {
    let source = Source {
        name: NameId::from_bytes([8; 16]),
        role: Role::FaceSet,
        flags: 0,
    };
    let run = || {
        reconstruct(
            mesh,
            Reconstruct::default(),
            source,
            &Exec::serial(),
            &mut WorkMeter::new(Budget::UNLIMITED),
        )
        .unwrap()
    };
    let first = run();
    let start = Instant::now();
    let runs = 5;
    for _ in 0..runs {
        std::hint::black_box(run());
    }
    let ms = start.elapsed().as_secs_f64() * 1e3 / f64::from(runs);
    let log = first.log;
    println!(
        "{name:<44} {:>5} triangles {:>5} cuts {:>5} cells {:>9.2} ms",
        log.triangles,
        log.cuts,
        log.cells.kept + log.cells.dropped + log.cells.undecided,
        ms
    );
}

fn main() {
    time(
        "a cube of 8 by 8 quads a face, a hole",
        &gridded_cube(8, 100),
    );
    time(
        "a cube of 16 by 16 quads a face, a hole",
        &gridded_cube(16, 100),
    );
    time("8 overlapping boxes", &pile(8, 100));
    time("16 overlapping boxes", &pile(16, 100));
}
