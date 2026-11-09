//! Shapes the exact mesh's tests build: boxes, a closed height field, a cut pyramid.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    dead_code,
    reason = "test shapes; each test file uses some of them"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_mesh::exact::build::{Corner, Polygons, Source};
use geom_mesh::exact::{PropTable, Role};
use geom_num::Exactness;
use geom_num::grid::P3;
use geom_num::plane::Plane;
use geom_op::NameId;

pub fn g(x: i64, y: i64, z: i64) -> Corner {
    Corner::Grid(P3::new(x, y, z).unwrap())
}

pub fn source(n: u8) -> Source {
    Source {
        name: NameId::from_bytes([n; 16]),
        role: Role::Face,
        flags: 0,
    }
}

/// Polygons of one run, each face one loop.
pub fn soup(vertices: Vec<Corner>, faces: &[&[u32]]) -> Polygons {
    let mut corners = Vec::new();
    let mut loop_start = vec![0];
    for f in faces {
        corners.extend_from_slice(f);
        loop_start.push(corners.len() as u32);
    }
    Polygons {
        vertices,
        corners,
        loop_start,
        face_loops: (0..=faces.len() as u32).collect(),
        face_run: vec![0; faces.len()],
        runs: vec![source(1)],
        face_plane: Vec::new(),
        edge_plane: Vec::new(),
        corner_prop: Vec::new(),
        props: PropTable::default(),
        face_dev: Vec::new(),
        class: Exactness::Exact,
    }
}

/// A box from the origin to (`a`, `b`, `c`), outward.
pub fn cuboid(a: i64, b: i64, c: i64) -> Polygons {
    soup(
        vec![
            g(0, 0, 0),
            g(a, 0, 0),
            g(a, b, 0),
            g(0, b, 0),
            g(0, 0, c),
            g(a, 0, c),
            g(a, b, c),
            g(0, b, c),
        ],
        &[
            &[0, 3, 2, 1],
            &[4, 5, 6, 7],
            &[0, 1, 5, 4],
            &[1, 2, 6, 5],
            &[2, 3, 7, 6],
            &[3, 0, 4, 7],
        ],
    )
}

/// A closed solid over a square of cells of side `step`, flat at z = 0 and with `heights` above,
/// which are positive, a row of a side at a time. With it, six times its volume, from its columns:
/// a triangle of area s²/2 with corners at heights a, b and c stands over 6 · (s²/2) · (a + b + c)/3.
pub fn prism(heights: &[Vec<i64>], step: i64) -> (Polygons, i128) {
    let n = heights.len() as u32;
    let at = |x: u32, y: u32| y * n + x;
    let mut vertices = Vec::new();
    for lift in [true, false] {
        for (y, row) in heights.iter().enumerate() {
            for (x, &height) in row.iter().enumerate() {
                let z = if lift { height } else { 0 };
                vertices.push(g(x as i64 * step, y as i64 * step, z));
            }
        }
    }
    let low = n * n;
    let mut faces: Vec<Vec<u32>> = Vec::new();
    let mut six = 0_i128;
    for y in 0..n - 1 {
        for x in 0..n - 1 {
            let (a, b, c, d) = (at(x, y), at(x + 1, y), at(x + 1, y + 1), at(x, y + 1));
            faces.push(vec![a, b, c]);
            faces.push(vec![a, c, d]);
            faces.push(vec![low + a, low + c, low + b]);
            faces.push(vec![low + a, low + d, low + c]);
            let h = |v: u32| i128::from(heights[(v / n) as usize][(v % n) as usize]);
            let square = i128::from(step) * i128::from(step);
            six += square * (h(a) + h(b) + h(c)) + square * (h(a) + h(c) + h(d));
        }
    }
    for i in 0..n - 1 {
        let walls = [
            (at(i, 0), at(i + 1, 0)),
            (at(n - 1, i), at(n - 1, i + 1)),
            (at(i + 1, n - 1), at(i, n - 1)),
            (at(0, i + 1), at(0, i)),
        ];
        for (a, b) in walls {
            faces.push(vec![low + a, low + b, b, a]);
        }
    }
    let refs: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
    (soup(vertices, &refs), six)
}

/// A square pyramid cut by the plane z = 1: its top corners are not grid points when the sides lean
/// by thirds. Four sides through the corners (±3, ±3, 0), meeting at (0, 0, 3), cut at z = 1 give
/// (±2, ±2, 1), on the grid; sides through (±2, ±2, 0) meeting at (0, 0, 3) give thirds.
pub fn frustum() -> Polygons {
    let bottom = |x: i64, y: i64| g(x, y, 0);
    let apex = P3::new(0, 0, 3).unwrap();
    let side = |a: (i64, i64), b: (i64, i64)| {
        Plane::through(
            P3::new(a.0, a.1, 0).unwrap(),
            P3::new(b.0, b.1, 0).unwrap(),
            apex,
        )
        .unwrap()
    };
    let top = Plane::through(
        P3::new(0, 0, 1).unwrap(),
        P3::new(1, 0, 1).unwrap(),
        P3::new(0, 1, 1).unwrap(),
    )
    .unwrap();
    let base = Plane::through(
        P3::new(0, 0, 0).unwrap(),
        P3::new(0, 1, 0).unwrap(),
        P3::new(1, 0, 0).unwrap(),
    )
    .unwrap();
    let (s, e, n, w) = (
        side((-2, -2), (2, -2)),
        side((2, -2), (2, 2)),
        side((2, 2), (-2, 2)),
        side((-2, 2), (-2, -2)),
    );
    let corners = vec![
        bottom(-2, -2),
        bottom(2, -2),
        bottom(2, 2),
        bottom(-2, 2),
        Corner::Planes([top, s, w]),
        Corner::Planes([top, s, e]),
        Corner::Planes([top, n, e]),
        Corner::Planes([top, n, w]),
    ];
    hexahedron(corners, &[base, top, s, e, n, w])
}

/// Both sets of polygons in one, the second's indices moved past the first's.
pub fn join(mut a: Polygons, b: &Polygons) -> Polygons {
    let vertices = a.vertices.len() as u32;
    let corners = a.corners.len() as u32;
    let loops = a.loop_start.len() as u32 - 1;
    a.vertices.extend(b.vertices.iter().copied());
    a.corners.extend(b.corners.iter().map(|c| c + vertices));
    a.loop_start
        .extend(b.loop_start.iter().skip(1).map(|l| l + corners));
    a.face_loops
        .extend(b.face_loops.iter().skip(1).map(|l| l + loops));
    a.face_run.extend(b.face_run.iter().copied());
    a.face_plane.extend(b.face_plane.iter().copied());
    a.edge_plane.extend(b.edge_plane.iter().copied());
    a
}

/// A slab 16 steps square from x = `x`, one part in 7 · 2^20 thick, between planes whose offsets
/// grow with `x` and differ by 1. Its coefficients in x, y and z are multiples of 7 and its offsets
/// are not, so no corner is on the grid.
pub fn slab(x: i64) -> Polygons {
    let n = [7 << 20, 14, 7];
    let upper = -(n[0] * i128::from(x)) - 6;
    let top = Plane::new(n, upper).unwrap();
    let base = Plane::new(n.map(|c| -c), -(upper + 1)).unwrap();
    let plane = |n: [i128; 3], d: i64| Plane::new(n, i128::from(d)).unwrap();
    let (s, e) = (plane([0, -1, 0], 0), plane([1, 0, 0], -(x + 16)));
    let (north, w) = (plane([0, 1, 0], -16), plane([-1, 0, 0], x));
    let at = |p: Plane, q: Plane| Corner::Planes([p, q, north]);
    let south = |p: Plane, q: Plane| Corner::Planes([p, q, s]);
    let corners = vec![
        south(base, w),
        south(base, e),
        at(base, e),
        at(base, w),
        south(top, w),
        south(top, e),
        at(top, e),
        at(top, w),
    ];
    hexahedron(corners, &[base, top, s, e, north, w])
}

/// A solid of six faces on these outward planes (under, over, then its four sides in turn), from its
/// corners, the four under it in turn and then the four over them.
pub fn hexahedron(vertices: Vec<Corner>, planes: &[Plane; 6]) -> Polygons {
    let &[base, top, s, e, n, w] = planes;
    let faces: [&[u32]; 6] = [
        &[0, 3, 2, 1],
        &[4, 5, 6, 7],
        &[0, 1, 5, 4],
        &[1, 2, 6, 5],
        &[2, 3, 7, 6],
        &[3, 0, 4, 7],
    ];
    let mut input = soup(vertices, &faces);
    input.face_plane = vec![Some(base), Some(top), Some(s), Some(e), Some(n), Some(w)];
    // Each side's edges lie on its neighbours; the top's and bottom's on the sides.
    let reversed = |p: Plane| Plane::new(p.n().map(|c| -c), -p.d()).unwrap();
    let inward = [reversed(s), reversed(e), reversed(n), reversed(w)];
    let (bs, be, bn, bw) = (inward[0], inward[1], inward[2], inward[3]);
    let up = base;
    let down = top;
    input.edge_plane = [
        // Bottom, 0-3, 3-2, 2-1, 1-0.
        [bw, bn, be, bs],
        // Top, 4-5, 5-6, 6-7, 7-4.
        [bs, be, bn, bw],
        // South, 0-1, 1-5, 5-4, 4-0.
        [reversed(up), be, reversed(down), bw],
        // East, 1-2, 2-6, 6-5, 5-1.
        [reversed(up), bn, reversed(down), bs],
        // North, 2-3, 3-7, 7-6, 6-2.
        [reversed(up), bw, reversed(down), be],
        // West, 3-0, 0-4, 4-7, 7-3.
        [reversed(up), bs, reversed(down), bn],
    ]
    .concat()
    .into_iter()
    .map(Some)
    .collect();
    input
}
