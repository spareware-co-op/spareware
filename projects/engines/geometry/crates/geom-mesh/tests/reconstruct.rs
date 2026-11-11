//! Reconstruction by winding number through the public interface (piece G299), against what 19.7
//! says of it: cells cut by the input's own planes, kept when the whole interval of the
//! generalised winding number is above one half, undecided cells reported and left out, the
//! result's faces on the input's planes where it bounds the result, and its class Approximate by a
//! deviation measured both ways.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::assert_is_empty,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "test shapes and checks; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_mesh::exact::build::Source;
use geom_mesh::exact::mass::Volume;
use geom_mesh::exact::raw::RawMesh;
use geom_mesh::exact::{ExactMesh, MeshKind, Role};
use geom_mesh::repair::{Reconstruct, ReconstructError, Reconstructed, reconstruct};
use geom_num::Exactness;
use geom_num::grid::{GridStep, P3};
use geom_num::wide::I256;
use geom_op::NameId;
use geom_run::{Budget, Exec, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 28))
}

fn source() -> Source {
    Source {
        name: NameId::from_bytes([5; 16]),
        role: Role::FaceSet,
        flags: 0,
    }
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn raw(points: &[[i64; 3]], faces: &[Vec<u32>]) -> RawMesh {
    let vertices = points.iter().map(|&[x, y, z]| p(x, y, z)).collect();
    let mut corners = Vec::new();
    let mut start = vec![0];
    for f in faces {
        corners.extend_from_slice(f);
        start.push(corners.len() as u32);
    }
    RawMesh::new(vertices, corners, start).unwrap()
}

fn run(mesh: &RawMesh) -> Result<Reconstructed, ReconstructError> {
    reconstruct(
        mesh,
        Reconstruct::default(),
        source(),
        &Exec::serial(),
        &mut meter(),
    )
}

fn six_volume(m: &ExactMesh) -> I256 {
    match m.volume(&mut meter()).unwrap() {
        Volume::Exact(six) => six,
        Volume::Certified(_) => panic!("every corner here is on the grid"),
    }
}

fn cube_volume(side: i64) -> I256 {
    I256::from_i128(6 * i128::from(side).pow(3))
}

/// The eight corners of the box from `low` to `high`, corner i at high on the axes of i's bits.
fn corners(low: [i64; 3], high: [i64; 3]) -> Vec<[i64; 3]> {
    (0..8)
        .map(|i| [0, 1, 2].map(|a| if i >> a & 1 == 1 { high[a] } else { low[a] }))
        .collect()
}

/// The six faces of a box on [`corners`], outward, each starting at `base`.
fn sides(base: u32) -> Vec<Vec<u32>> {
    [
        [0, 4, 6, 2],
        [1, 3, 7, 5],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 2, 3, 1],
        [4, 5, 7, 6],
    ]
    .iter()
    .map(|f| f.iter().map(|&v| v + base).collect())
    .collect()
}

fn cube(side: i64) -> RawMesh {
    raw(&corners([0; 3], [side; 3]), &sides(0))
}

fn approximate(r: &Reconstructed) -> u64 {
    match r.mesh.parts().class {
        Exactness::Approximate { deviation, step } => {
            assert_eq!(step, GridStep::Q0);
            deviation.get()
        }
        other => panic!("not approximate: {other:?}"),
    }
}

#[test]
fn a_closed_cube_comes_back_as_itself_with_a_deviation_of_the_rounding_alone() {
    let r = run(&cube(100)).unwrap();
    assert!(r.mesh.is_solid());
    assert_ne!(r.mesh.kind(), MeshKind::Open);
    assert_eq!(six_volume(&r.mesh), cube_volume(100));
    assert_eq!(r.mesh.faces(), 6);
    assert_eq!(r.log.cells.kept, 1);
    assert_eq!(r.log.cells.undecided, 0);
    assert_eq!(r.undecided, Vec::new());
    // Every corner is on the grid and on the input: the one step is the rounding's allowance.
    assert_eq!(approximate(&r), 1);
}

#[test]
fn a_cube_turned_inside_out_is_turned_outward_first() {
    let mut faces = sides(0);
    for f in &mut faces {
        f.reverse();
    }
    let r = run(&raw(&corners([0; 3], [100; 3]), &faces)).unwrap();
    assert_eq!(six_volume(&r.mesh), cube_volume(100));
}

/// A cube whose bottom is a frame of four trapezoids round a square hole.
fn cube_with_a_hole() -> RawMesh {
    let mut points = corners([0; 3], [100; 3]);
    // The hole's corners, in the bottom plane, from 25 to 75.
    for (x, y) in [(25, 25), (75, 25), (75, 75), (25, 75)] {
        points.push([x, y, 0]);
    }
    let mut faces = sides(0);
    // The bottom is the face whose corners are all at z = 0.
    faces.retain(|f| !f.iter().all(|&v| points[v as usize][2] == 0));
    // The bottom's outer corners are 0 (0,0), 1 (100,0), 3 (100,100), 2 (0,100); seen from below,
    // facing down, the frame goes round the other way from the order above.
    let outer = [0_u32, 2, 3, 1];
    let inner = [8_u32, 11, 10, 9];
    for k in 0..4 {
        let (a, b) = (outer[k], outer[(k + 1) % 4]);
        let (c, d) = (inner[(k + 1) % 4], inner[k]);
        faces.push(vec![a, b, c, d]);
    }
    raw(&points, &faces)
}

#[test]
fn a_hole_in_a_flat_face_is_closed_in_the_faces_own_plane() {
    let r = run(&cube_with_a_hole()).unwrap();
    assert!(r.mesh.is_solid());
    assert_eq!(six_volume(&r.mesh), cube_volume(100));
    assert_eq!(r.mesh.faces(), 6);
    assert_eq!(r.undecided, Vec::new());
    // The hole is 50 wide and its filling lies in the frame's plane: the input is within the
    // rounding of the result, and the result within the rounding of the input.
    assert_eq!(approximate(&r), 1);
}

#[test]
fn two_overlapping_closed_boxes_give_their_union() {
    let mut points = corners([0; 3], [100; 3]);
    points.extend(corners([50; 3], [150; 3]));
    let mut faces = sides(0);
    faces.extend(sides(8));
    let r = run(&raw(&points, &faces)).unwrap();
    assert!(r.mesh.is_solid());
    let union = 2 * 1_000_000 - 50_i128.pow(3);
    assert_eq!(six_volume(&r.mesh), I256::from_i128(6 * union));
}

#[test]
fn faces_given_in_another_order_give_the_same_mesh() {
    let m = cube_with_a_hole();
    let a = run(&m).unwrap();
    // The same faces, last first, and each one's corners started from another.
    let mut faces: Vec<Vec<u32>> = Vec::new();
    let n = m.face_start().len() - 1;
    for f in (0..n).rev() {
        let s = m.face_start()[f] as usize;
        let e = m.face_start()[f + 1] as usize;
        let mut c = m.corners()[s..e].to_vec();
        c.rotate_left(1);
        faces.push(c);
    }
    let points: Vec<[i64; 3]> = m
        .vertices()
        .iter()
        .map(|v| [v.x.get(), v.y.get(), v.z.get()])
        .collect();
    let b = run(&raw(&points, &faces)).unwrap();
    assert_eq!(a.mesh, b.mesh);
    assert_eq!(a.log, b.log);
}

#[test]
fn a_cell_too_thin_to_place_a_point_in_is_undecided_and_left_out() {
    // Two squares one step apart: the slab between them is narrower than the box round its
    // centroid, which therefore holds points of both, where the winding number jumps.
    let points = [
        [0, 0, 0],
        [100, 0, 0],
        [100, 100, 0],
        [0, 100, 0],
        [0, 0, 1],
        [100, 0, 1],
        [100, 100, 1],
        [0, 100, 1],
    ];
    let faces = vec![vec![0, 3, 2, 1], vec![4, 5, 6, 7]];
    match run(&raw(&points, &faces)) {
        Err(ReconstructError::NothingKept { undecided }) => {
            assert_eq!(undecided.len(), 1);
            let slab = undecided[0];
            // The box holds points of both squares, so the interval is wide: it holds a half.
            assert!(slab.low <= 0.5 && slab.high >= 0.5, "{slab:?}");
            assert!((0..=1).contains(&slab.at.z.get()));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn faces_with_no_area_are_no_surface() {
    let points = [[0, 0, 0], [10, 0, 0], [20, 0, 0]];
    let e = run(&raw(&points, &[vec![0, 1, 2]])).unwrap_err();
    assert_eq!(e, ReconstructError::NoSurface);
}

#[test]
fn a_surface_that_encloses_nothing_keeps_no_cell() {
    // One triangle: the winding number nowhere reaches a half.
    let points = [[0, 0, 0], [100, 0, 0], [0, 100, 0]];
    match run(&raw(&points, &[vec![0, 1, 2]])) {
        Err(ReconstructError::NothingKept { .. }) => {}
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_small_budget_stops_it() {
    let mut tiny = WorkMeter::new(Budget::of_work(10));
    let e = reconstruct(
        &cube(100),
        Reconstruct::default(),
        source(),
        &Exec::serial(),
        &mut tiny,
    )
    .unwrap_err();
    assert!(
        matches!(e, ReconstructError::Mesh(_) | ReconstructError::Heal(_)),
        "{e:?}"
    );
}
