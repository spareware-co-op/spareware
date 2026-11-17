//! Exact meshes as faceted B-reps through the public interface (piece G149): every body passes the
//! B-rep checker, and adjacent faces on one plane become one.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_lossless,
    reason = "test checks; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeSet;

use geom_check::{Level, check};
use geom_mesh::exact::build::{Polygons, build};
use geom_mesh::exact::{ExactMesh, MeshError};
use geom_mesh::faceted::faceted;
use geom_num::Exactness;
use geom_run::{Budget, Exec, WorkMeter};
use geom_topo::Body;

mod shapes;

use shapes::{cuboid, frustum, g, prism, soup};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

fn mesh(input: &Polygons) -> ExactMesh {
    build(input, &Exec::serial(), &mut meter()).unwrap()
}

/// `input` as a body the checker passes.
fn body(input: &Polygons) -> Body {
    let body = faceted(&mesh(input), &mut meter()).unwrap();
    let report = check(&body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert!(report.passed(), "{report:?}");
    body
}

/// `input` with each quad split in two, so that each side of a box is two faces on one plane.
fn split(input: &Polygons) -> Polygons {
    let mut faces: Vec<Vec<u32>> = Vec::new();
    for pair in input.loop_start.windows(2) {
        let ring = &input.corners[pair[0] as usize..pair[1] as usize];
        faces.push(vec![ring[0], ring[1], ring[2]]);
        faces.push(vec![ring[0], ring[2], ring[3]]);
    }
    let refs: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
    soup(input.vertices.clone(), &refs)
}

#[test]
fn a_box_in_triangles_is_a_box_of_six_faces() {
    let input = split(&cuboid(4, 6, 8));
    assert_eq!(mesh(&input).faces(), 12);
    let b = body(&input);
    assert_eq!(b.arenas().faces.len(), 6);
    assert_eq!(b.arenas().edges.len(), 12);
    assert_eq!(b.arenas().vertices.len(), 8);
}

#[test]
fn a_rough_solid_keeps_its_faces_and_its_flat_base_merges() {
    let heights = vec![
        vec![40, 56, 48, 64],
        vec![72, 40, 80, 48],
        vec![48, 64, 56, 72],
        vec![56, 48, 64, 40],
    ];
    let (input, _) = prism(&heights, 64);
    let m = mesh(&input);
    let b = body(&input);
    // Each plane of this solid is one connected face: its base and walls, and its rough top.
    let planes: BTreeSet<(u32, bool)> = m
        .parts()
        .face_plane
        .iter()
        .map(|f| (f.row, f.flipped))
        .collect();
    assert!(m.faces() > planes.len());
    assert_eq!(b.arenas().faces.len() as usize, planes.len());
}

#[test]
fn a_tilted_solid_passes() {
    body(&soup(
        vec![g(0, 0, 0), g(30, 1, 2), g(3, 50, 7), g(11, 13, 70)],
        &[&[0, 2, 1], &[0, 1, 3], &[1, 2, 3], &[2, 0, 3]],
    ));
}

#[test]
fn corners_off_the_grid_stay_three_planes() {
    let b = body(&frustum());
    assert_eq!(b.arenas().faces.len(), 6);
}

#[test]
fn an_open_mesh_is_refused() {
    let open = mesh(&soup(
        vec![g(0, 0, 0), g(4, 0, 0), g(0, 4, 0)],
        &[&[0, 1, 2]],
    ));
    assert_eq!(
        faceted(&open, &mut meter()).unwrap_err(),
        MeshError::NotClosed
    );
}
