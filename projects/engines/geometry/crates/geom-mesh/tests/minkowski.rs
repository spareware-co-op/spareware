//! Minkowski sums through the public interface (piece G150), exact against closed forms on boxes
//! and prisms, and against counted cells on an L.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_wrap,
    reason = "test checks; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_mesh::exact::build::{Corner, Polygons, build};
use geom_mesh::exact::mass::Volume;
use geom_mesh::exact::{ExactMesh, MeshError, MeshKind};
use geom_mesh::minkowski::{Sum, minkowski};
use geom_num::Exactness;
use geom_num::grid::GridStep;
use geom_num::wide::I256;
use geom_run::{Budget, Exec, WorkMeter};

mod shapes;

use shapes::{cuboid, frustum, g, soup, source};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 28))
}

fn mesh(input: &Polygons) -> ExactMesh {
    build(input, &Exec::serial(), &mut meter()).unwrap()
}

fn sum(a: &ExactMesh, b: &ExactMesh) -> Sum {
    minkowski(
        a,
        b,
        (source(9), GridStep::Q0),
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap()
}

fn six_volume(m: &ExactMesh) -> I256 {
    match m.volume(&mut meter()).unwrap() {
        Volume::Exact(six) => six,
        Volume::Certified(_) => panic!("every corner here is on the grid"),
    }
}

fn six(v: i128) -> I256 {
    I256::from_i128(6 * v)
}

/// A prism of `outline`, counter-clockwise from above, from z = 0 to `height`.
fn prism(outline: &[(i64, i64)], height: i64) -> Polygons {
    let n = outline.len() as u32;
    let mut vertices = Vec::new();
    for z in [0, height] {
        for &(x, y) in outline {
            vertices.push(g(x, y, z));
        }
    }
    let mut faces: Vec<Vec<u32>> = vec![(0..n).rev().collect(), (n..2 * n).collect()];
    for i in 0..n {
        let j = (i + 1) % n;
        faces.push(vec![i, j, j + n, i + n]);
    }
    let refs: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
    soup(vertices, &refs)
}

fn moved(mut input: Polygons, x: i64, y: i64, z: i64) -> Polygons {
    for v in &mut input.vertices {
        let Corner::Grid(p) = *v else {
            panic!("these shapes are on the grid");
        };
        *v = g(p.x.get() + x, p.y.get() + y, p.z.get() + z);
    }
    input
}

#[test]
fn two_boxes_add_their_sides() {
    let a = mesh(&moved(cuboid(2, 3, 4), 1, 1, 1));
    let b = mesh(&moved(cuboid(5, 1, 2), -3, 0, 7));
    let s = sum(&a, &b);
    assert_eq!(s.class, Exactness::Exact);
    assert_eq!(s.mesh.kind(), MeshKind::Closed);
    assert_eq!(s.mesh.faces(), 6);
    assert_eq!(six_volume(&s.mesh), six(7 * 4 * 6));
}

#[test]
fn a_triangular_prism_and_a_box() {
    // The sum of a right triangle of legs 4 and a square of side 2: its area, 8, plus the sides
    // swept, 4 · 2 + 4 · 2, plus the square, 4; the heights add.
    let a = mesh(&prism(&[(0, 0), (4, 0), (0, 4)], 3));
    let b = mesh(&cuboid(2, 2, 5));
    let s = sum(&a, &b);
    assert_eq!(s.mesh.kind(), MeshKind::Closed);
    assert_eq!(six_volume(&s.mesh), six((8 + 8 + 8 + 4) * 8));
}

#[test]
fn an_l_and_a_box_against_counted_cells() {
    // An L of unit cells, its sum with a box of cells: the cells any translate reaches.
    let ell = [(0, 0), (4, 0), (4, 1), (1, 1), (1, 4), (0, 4)];
    let a = mesh(&prism(&ell, 2));
    let b = mesh(&cuboid(2, 2, 1));
    let s = sum(&a, &b);
    assert_eq!(s.mesh.kind(), MeshKind::Closed);
    let cells: Vec<(i64, i64)> = (0..4)
        .flat_map(|x| (0..4).map(move |y| (x, y)))
        .filter(|&(x, y)| x == 0 || y == 0)
        .collect();
    let mut reached = std::collections::BTreeSet::new();
    for (x, y) in cells {
        for dx in 0..=2 {
            for dy in 0..=2 {
                reached.insert((x + dx, y + dy));
            }
        }
    }
    assert_eq!(six_volume(&s.mesh), six(reached.len() as i128 * 3));
    assert_eq!(reached.len(), 27);
}

#[test]
fn sums_commute() {
    let ell = [(0, 0), (4, 0), (4, 1), (1, 1), (1, 4), (0, 4)];
    let a = mesh(&prism(&ell, 2));
    let b = mesh(&prism(&[(0, 0), (3, 0), (0, 3)], 1));
    assert_eq!(sum(&a, &b).mesh, sum(&b, &a).mesh);
}

#[test]
fn a_solid_off_the_grid_commits_first() {
    let a = mesh(&frustum());
    let b = mesh(&cuboid(2, 2, 2));
    let s = sum(&a, &b);
    assert_eq!(
        s.class,
        Exactness::Certified {
            bound: geom_num::grid::Steps(1),
            step: GridStep::Q0
        }
    );
    assert_eq!(s.mesh.kind(), MeshKind::Closed);
}

#[test]
fn an_open_mesh_is_refused() {
    let open = mesh(&soup(
        vec![g(0, 0, 0), g(4, 0, 0), g(0, 4, 0)],
        &[&[0, 1, 2]],
    ));
    let ell = [(0, 0), (4, 0), (4, 1), (1, 1), (1, 4), (0, 4)];
    let solid = mesh(&prism(&ell, 2));
    let refused = minkowski(
        &solid,
        &open,
        (source(9), GridStep::Q0),
        &Exec::serial(),
        &mut meter(),
    );
    assert_eq!(refused.unwrap_err(), MeshError::NotClosed);
    let refused = minkowski(
        &open,
        &solid,
        (source(9), GridStep::Q0),
        &Exec::serial(),
        &mut meter(),
    );
    assert_eq!(refused.unwrap_err(), MeshError::NotClosed);
}

#[test]
fn convex_solids_take_one_hull() {
    let a = mesh(&cuboid(2, 3, 4));
    let b = mesh(&moved(cuboid(5, 1, 2), 1, 1, 1));
    let mut counted = WorkMeter::new(Budget::of_work(1 << 28));
    let s = minkowski(
        &a,
        &b,
        (source(9), GridStep::Q0),
        &Exec::serial(),
        &mut counted,
    )
    .unwrap();
    assert_eq!(six_volume(&s.mesh), six(7 * 4 * 6));
    // One hull of 64 points: far under the 144 hulls and their union of solids that are not convex.
    assert!(counted.used().work < 1 << 12);
}

#[test]
fn a_meter_that_runs_out_stops_the_sum() {
    let a = mesh(&cuboid(2, 3, 4));
    let refused = minkowski(
        &a,
        &a,
        (source(9), GridStep::Q0),
        &Exec::serial(),
        &mut WorkMeter::new(Budget::of_work(1)),
    );
    assert!(matches!(refused, Err(MeshError::Stopped(_))));
}

#[test]
fn a_sum_past_the_frame_is_refused() {
    let far = (1 << 39) + (1 << 38);
    let a = mesh(&moved(cuboid(2, 2, 2), far, 0, 0));
    let refused = minkowski(
        &a,
        &a,
        (source(9), GridStep::Q0),
        &Exec::serial(),
        &mut meter(),
    );
    assert!(matches!(refused, Err(MeshError::Num(_))));
}
