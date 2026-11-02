//! The exact mesh through the public interface (piece G146), against what 19.2 says of it: the
//! checker refuses each fault, the order of the input does not show, edges pair in radial order,
//! and volumes are exact.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "test checks and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroU64;

use geom_mesh::exact::build::{Corner, Polygons, build};
use geom_mesh::exact::mass::Volume;
use geom_mesh::exact::raw::RawMesh;
use geom_mesh::exact::{
    ExactMesh, MeshError, MeshFault, MeshKind, NONE, Parts, PropTable, Vertex, morton,
};
use geom_num::grid::{GridStep, P3, Steps};
use geom_num::plane::{Plane, Tpi};
use geom_num::wide::I256;
use geom_num::{Exactness, NumError, Sign};
use geom_op::NameId;
use geom_run::{Budget, BudgetKind, Exec, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

mod shapes;

use shapes::{cuboid, frustum, g, join, prism, slab, soup, source};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 20))
}

fn mesh(input: &Polygons) -> ExactMesh {
    build(input, &Exec::serial(), &mut meter()).unwrap()
}

fn six(m: &ExactMesh) -> i128 {
    match m.volume(&mut meter()).unwrap() {
        Volume::Exact(v) => v.to_i128().unwrap(),
        Volume::Certified(_) => panic!("off the grid"),
    }
}

/// A fault's name, from a change to a good mesh.
fn fault(change: impl FnOnce(&mut Parts)) -> MeshError {
    let mut p = mesh(&cuboid(2, 3, 4)).into_parts();
    change(&mut p);
    ExactMesh::new(p, &Exec::serial(), &mut meter()).unwrap_err()
}

#[test]
fn a_box_is_closed_and_its_volume_exact() {
    let m = mesh(&cuboid(2, 3, 4));
    assert_eq!(m.kind(), MeshKind::Closed);
    assert_eq!(m.faces(), 6);
    assert_eq!(six(&m), 6 * 24);
    let p = m.parts();
    assert_eq!(p.verts.len(), 8);
    assert_eq!(p.he_vert.len(), 24);
    // Six face planes, and every edge plane is a neighbour's face plane or its own line's.
    assert!(p.planes.len() >= 6);
    assert!(p.he_pair.iter().all(|&h| h != NONE));
}

#[test]
fn volume_is_exact_at_the_edge_of_the_frame() {
    let e = (1_i64 << 40) - 1;
    let m = mesh(&cuboid(e, e, e));
    assert_eq!(six(&m), 6 * i128::from(e) * i128::from(e) * i128::from(e));
}

#[test]
fn the_order_of_the_input_does_not_show() {
    let want = mesh(&cuboid(2, 3, 4));
    let input = cuboid(2, 3, 4);
    // Reversed vertices, reversed faces, and each loop turned one place.
    let n = input.vertices.len() as u32;
    let vertices: Vec<Corner> = input.vertices.iter().rev().copied().collect();
    let faces: Vec<Vec<u32>> = input
        .corners
        .chunks(4)
        .rev()
        .map(|f| {
            let mut f: Vec<u32> = f.iter().map(|v| n - 1 - v).collect();
            f.rotate_left(1);
            f
        })
        .collect();
    let refs: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
    assert_eq!(mesh(&soup(vertices, &refs)), want);
}

#[test]
fn repeated_grid_points_are_one_vertex() {
    let mut input = cuboid(2, 3, 4);
    input.vertices.push(g(0, 0, 0));
    let last = input.vertices.len() as u32 - 1;
    input.corners[0] = last;
    let m = mesh(&input);
    assert_eq!(m.parts().verts.len(), 8);
    assert_eq!(m.kind(), MeshKind::Closed);
}

#[test]
fn an_open_surface_has_boundary_edges() {
    let m = mesh(&soup(
        vec![g(0, 0, 0), g(4, 0, 0), g(4, 4, 0), g(0, 4, 0)],
        &[&[0, 1, 2], &[0, 2, 3]],
    ));
    assert_eq!(m.kind(), MeshKind::Open);
    assert_eq!(m.parts().he_pair.iter().filter(|&&h| h == NONE).count(), 4);
}

#[test]
fn a_face_keeps_its_hole() {
    // A square of side 8 with a square hole of side 2, in the plane z = 0, facing up.
    let mut input = soup(
        vec![
            g(0, 0, 0),
            g(8, 0, 0),
            g(8, 8, 0),
            g(0, 8, 0),
            g(3, 3, 0),
            g(3, 5, 0),
            g(5, 5, 0),
            g(5, 3, 0),
        ],
        &[&[0, 1, 2, 3], &[4, 5, 6, 7]],
    );
    input.face_loops = vec![0, 2];
    input.face_run = vec![0];
    let m = mesh(&input);
    assert_eq!(m.faces(), 1);
    assert_eq!(m.twice_area_vector(0).unwrap(), Some([0, 0, 2 * (64 - 4)]));
    let area = m.area(0).unwrap().unwrap();
    assert!(area.contains(60.0));
}

#[test]
fn two_boxes_on_one_edge_pair_in_radial_order() {
    // Boxes meeting only along the edge from (0, 0, 0) to (0, 0, 2).
    let boxes = |x: i64, y: i64, base: u32| -> (Vec<Corner>, Vec<Vec<u32>>) {
        let (a, b) = (x + 2, y + 2);
        let vertices = vec![
            g(x, y, 0),
            g(a, y, 0),
            g(a, b, 0),
            g(x, b, 0),
            g(x, y, 2),
            g(a, y, 2),
            g(a, b, 2),
            g(x, b, 2),
        ];
        let faces: [[u32; 4]; 6] = [
            [0, 3, 2, 1],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
        ];
        (
            vertices,
            faces
                .iter()
                .map(|f| f.iter().map(|v| v + base).collect())
                .collect(),
        )
    };
    let (mut vertices, mut faces) = boxes(0, 0, 0);
    let (other, more) = boxes(-2, -2, 8);
    vertices.extend(other);
    faces.extend(more);
    let refs: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
    let m = mesh(&soup(vertices, &refs));
    assert_eq!(m.kind(), MeshKind::General);
    assert_eq!(six(&m), 2 * 6 * 8);
    // Every pair stays on one box: each pair bounds one solid. A face's far corner says which.
    let p = m.parts();
    let place = |h: usize| match p.verts[p.he_vert[h] as usize] {
        Vertex::Grid(at) => at.x.get() + at.y.get(),
        Vertex::Planes(_) => 0,
    };
    let solid = |h: usize| place(next_half(p, next_half(p, h)));
    for (h, &q) in p.he_pair.iter().enumerate() {
        assert_eq!(p.he_pair[q as usize] as usize, h);
        let (a, b) = (solid(h) + place(h), solid(q as usize) + place(q as usize));
        assert!(a * b >= 0, "{h} {q}");
    }
    // A shared edge's pairs are checked too: either half of a pair changed alone is refused.
    let shared: Vec<usize> = (0..p.he_pair.len())
        .filter(|&h| place(h) == 0 && place(next_half(p, h)) == 0)
        .collect();
    assert_eq!(shared.len(), 4);
    for &h in &shared {
        let other = shared
            .iter()
            .copied()
            .find(|&o| o != h && o != p.he_pair[h] as usize)
            .unwrap();
        let mut wrong = p.clone();
        wrong.he_pair[h] = other as u32;
        assert!(matches!(
            ExactMesh::new(wrong, &Exec::serial(), &mut meter()),
            Err(MeshError::Fault(MeshFault::RadialOrder { .. }))
        ));
    }
}

/// The half-edge after `h` round its loop.
fn next_half(p: &Parts, h: usize) -> usize {
    let h = h as u32;
    let looped = p.loop_start.partition_point(|&s| s <= h) - 1;
    let next = h + 1;
    if next == p.loop_start[looped + 1] {
        p.loop_start[looped] as usize
    } else {
        next as usize
    }
}

#[test]
fn three_planes_off_the_grid_stay_three_planes() {
    let m = mesh(&frustum());
    let p = m.parts();
    assert_eq!(m.kind(), MeshKind::Closed);
    assert_eq!(
        p.verts
            .iter()
            .filter(|v| matches!(v, Vertex::Planes(_)))
            .count(),
        4
    );
    // The frustum of side 4 to side 4/3 and height 1, of the cone of height 3: 16 · 3 / 3 less
    // (16/9) · 2 ... by similar solids, (1 − (2/3)³) of the whole, whose volume is 16.
    let want = 6.0 * 16.0 * (1.0 - 8.0 / 27.0);
    match m.volume(&mut meter()).unwrap() {
        Volume::Certified(v) => {
            assert!(v.contains(want), "{v:?}");
            assert!(v.hi() - v.lo() < 1e-9);
        }
        Volume::Exact(_) => panic!("thirds are not on the grid"),
    }
}

#[test]
fn three_planes_on_the_grid_become_a_grid_point() {
    let x = Plane::new([1, 0, 0], -2).unwrap();
    let y = Plane::new([0, 1, 0], 0).unwrap();
    let z = Plane::new([0, 0, 1], 0).unwrap();
    let input = soup(
        vec![
            g(0, 0, 0),
            Corner::Planes([x, y, z]),
            g(2, 2, 0),
            g(0, 2, 0),
        ],
        &[&[0, 1, 2], &[0, 2, 3]],
    );
    let m = mesh(&input);
    assert!(m.parts().verts.iter().all(|v| matches!(v, Vertex::Grid(_))));
}

#[test]
fn a_face_off_the_grid_needs_its_plane() {
    let mut input = frustum();
    input.face_plane = Vec::new();
    assert!(matches!(
        build(&input, &Exec::serial(), &mut meter()),
        Err(MeshError::FacePlaneNeeded { .. })
    ));
    let mut input = frustum();
    input.edge_plane = Vec::new();
    assert!(matches!(
        build(&input, &Exec::serial(), &mut meter()),
        Err(MeshError::EdgePlaneNeeded { .. })
    ));
}

#[test]
fn the_checker_refuses_each_fault() {
    let shape = MeshError::Fault(MeshFault::Shape);
    assert_eq!(
        fault(|p| {
            p.he_plane.pop();
        }),
        shape
    );
    assert_eq!(fault(|p| p.he_vert[0] = 100), shape);
    assert_eq!(fault(|p| p.he_pair[0] = 100), shape);
    assert_eq!(fault(|p| p.he_prop[0] = 0), shape);
    assert_eq!(fault(|p| p.face_plane[0].row = 10_000), shape);
    assert_eq!(fault(|p| p.loop_start[1] = 100), shape);
    assert_eq!(fault(|p| p.face_loops[1] = 0), shape);
    assert_eq!(fault(|p| p.runs[0].faces.end = 2), shape);
    assert_eq!(fault(|p| p.runs[0].faces.start = 1), shape);
    assert_eq!(fault(|p| p.loop_start[0] = 1), shape);
    // A loop no face holds, at the end of the arrays.
    assert_eq!(
        fault(|p| {
            let end = p.he_vert.len() as u32;
            let first: Vec<_> = p.he_plane[..3].to_vec();
            p.he_vert.extend([0, 1, 2]);
            p.he_plane.extend(first);
            p.he_pair.extend([NONE; 3]);
            p.he_prop.extend([NONE; 3]);
            p.loop_start.push(end + 3);
        }),
        shape
    );
    // An edge's plane through one of its two ends is off the edge.
    for other in [1, 3] {
        assert_eq!(
            fault(|p| p.he_plane[0] = p.he_plane[other]),
            MeshError::Fault(MeshFault::OffEdgePlane { half_edge: 0 })
        );
    }

    assert!(matches!(
        fault(|p| p.planes.swap(0, 1)),
        MeshError::Fault(MeshFault::PlanesOutOfOrder { row: 1 })
    ));
    assert!(matches!(
        fault(|p| {
            let n = p.planes[0].n().map(|c| c * 2);
            p.planes[0] = Plane::new(n, p.planes[0].d() * 2).unwrap();
        }),
        MeshError::Fault(MeshFault::PlaneNotCanonical { row: 0 })
    ));
    assert!(matches!(
        fault(|p| {
            let n = p.planes[0].n().map(|c| -c);
            p.planes[0] = Plane::new(n, -p.planes[0].d()).unwrap();
        }),
        MeshError::Fault(MeshFault::PlaneNotCanonical { row: 0 })
    ));
    assert!(matches!(
        fault(|p| p.verts.swap(0, 1)),
        MeshError::Fault(MeshFault::VerticesOutOfOrder { vertex: 1 })
    ));
    assert!(matches!(
        fault(|p| p.face_dev[3] = Steps(1)),
        MeshError::Fault(MeshFault::Deviation { face: 3 })
    ));
    assert!(matches!(
        fault(|p| p.kind = MeshKind::Open),
        MeshError::Fault(MeshFault::WrongKind)
    ));
    assert!(matches!(
        fault(|p| p.he_pair[0] = NONE),
        MeshError::Fault(MeshFault::BadPair { half_edge: _ })
    ));
    assert!(matches!(
        fault(|p| p.face_plane[0].flipped = !p.face_plane[0].flipped),
        MeshError::Fault(MeshFault::EdgePlaneFacing { .. } | MeshFault::WrongWayRound { .. })
    ));
    assert!(matches!(
        fault(|p| p.he_plane[0].flipped = !p.he_plane[0].flipped),
        MeshError::Fault(MeshFault::EdgePlaneFacing { half_edge: 0 })
    ));
    assert!(matches!(
        fault(|p| p.he_plane[0].row = p.face_plane[0].row),
        MeshError::Fault(MeshFault::EdgePlaneFacing { half_edge: 0 })
    ));
    assert!(matches!(
        fault(|p| {
            let row = p.he_plane[0].row;
            p.face_plane[0].row = row;
        }),
        MeshError::Fault(MeshFault::OffFacePlane { .. } | MeshFault::EdgePlaneFacing { .. })
    ));
    assert!(matches!(
        fault(|p| {
            let other = p.he_plane[2].row;
            p.he_plane[0].row = other;
        }),
        MeshError::Fault(MeshFault::OffEdgePlane { half_edge: 0 })
    ));
    assert!(matches!(
        fault(|p| p.he_vert[1] = p.he_vert[0]),
        MeshError::Fault(MeshFault::LoopStart { .. } | MeshFault::ZeroEdge { .. })
    ));
    assert!(matches!(
        fault(|p| p.he_vert[..4].rotate_left(1)),
        MeshError::Fault(MeshFault::LoopStart { looped: 0 })
    ));
    assert!(matches!(
        fault(|p| {
            let (a, b) = p.he_vert.split_at_mut(4);
            a[..4].swap_with_slice(&mut b[..4]);
        }),
        MeshError::Fault(MeshFault::FacesOutOfOrder { .. })
    ));
    let name = NameId::from_bytes([9; 16]);
    assert!(matches!(
        fault(|p| {
            let second = p.runs[0].clone();
            p.runs[0].faces.end = 3;
            p.runs.push(geom_mesh::exact::Run {
                faces: 3..6,
                source: name,
                ..second
            });
            p.runs.swap(0, 1);
            p.runs[0].faces = 0..3;
            p.runs[1].faces = 3..6;
        }),
        MeshError::Fault(MeshFault::RunsOutOfOrder { .. })
    ));
}

#[test]
fn a_mesh_with_one_face_reversed_is_refused() {
    let mut input = cuboid(2, 3, 4);
    input.corners[..4].reverse();
    assert!(matches!(
        build(&input, &Exec::serial(), &mut meter()),
        Err(MeshError::Fault(MeshFault::UnevenEdge { .. }))
    ));
}

#[test]
fn two_holes_stay_in_key_order() {
    let mut input = soup(
        vec![
            g(0, 0, 0),
            g(20, 0, 0),
            g(20, 20, 0),
            g(0, 20, 0),
            g(2, 2, 0),
            g(2, 4, 0),
            g(4, 4, 0),
            g(4, 2, 0),
            g(12, 12, 0),
            g(12, 14, 0),
            g(14, 14, 0),
            g(14, 12, 0),
        ],
        &[&[0, 1, 2, 3], &[8, 9, 10, 11], &[4, 5, 6, 7]],
    );
    input.face_loops = vec![0, 3];
    input.face_run = vec![0];
    let m = mesh(&input);
    let mut p = m.into_parts();
    let holes: Vec<u32> = p.loop_start[1..3]
        .iter()
        .map(|&h| p.he_vert[h as usize])
        .collect();
    assert!(holes[0] < holes[1]);
    p.he_vert[4..12].rotate_left(4);
    let e = ExactMesh::new(p, &Exec::serial(), &mut meter()).unwrap_err();
    assert!(matches!(
        e,
        MeshError::Fault(MeshFault::HolesOutOfOrder { face: 0 })
    ));
}

#[test]
fn a_repeated_face_is_refused() {
    let input = soup(
        vec![g(0, 0, 0), g(4, 0, 0), g(0, 4, 0)],
        &[&[0, 1, 2], &[1, 2, 0]],
    );
    assert!(build(&input, &Exec::serial(), &mut meter()).is_err());
}

#[test]
fn a_flat_face_is_refused() {
    let input = soup(vec![g(0, 0, 0), g(1, 1, 1), g(2, 2, 2)], &[&[0, 1, 2]]);
    assert!(matches!(
        build(&input, &Exec::serial(), &mut meter()),
        Err(MeshError::Fault(MeshFault::DegenerateFace { face: 0 }))
    ));
    let input = soup(vec![g(0, 0, 0), g(1, 1, 1)], &[&[0, 1, 0]]);
    assert!(build(&input, &Exec::serial(), &mut meter()).is_err());
}

#[test]
fn a_short_loop_is_refused() {
    let input = soup(vec![g(0, 0, 0), g(1, 0, 0), g(0, 1, 0)], &[&[0, 1]]);
    assert!(matches!(
        build(&input, &Exec::serial(), &mut meter()),
        Err(MeshError::Fault(MeshFault::ShortLoop { looped: 0 }))
    ));
}

#[test]
fn class_bounds_the_deviations() {
    let step = GridStep::Q0;
    let mut input = cuboid(2, 3, 4);
    input.face_dev = vec![Steps(3); 6];
    assert!(matches!(
        build(&input, &Exec::serial(), &mut meter()),
        Err(MeshError::Fault(MeshFault::Deviation { face: 0 }))
    ));
    input.class = Exactness::Certified {
        bound: Steps(3),
        step,
    };
    assert_eq!(mesh(&input).parts().face_dev, vec![Steps(3); 6]);
    input.class = Exactness::Certified {
        bound: Steps(2),
        step,
    };
    assert!(build(&input, &Exec::serial(), &mut meter()).is_err());
}

#[test]
fn runs_sort_by_source_and_cover_the_faces() {
    let mut input = cuboid(2, 3, 4);
    input.runs = vec![source(5), source(2)];
    input.face_run = vec![0, 0, 0, 1, 1, 1];
    let p = mesh(&input).into_parts();
    assert_eq!(p.runs.len(), 2);
    assert_eq!(p.runs[0].source, source(2).name);
    assert_eq!(p.runs[0].faces, 0..3);
    assert_eq!(p.runs[1].faces, 3..6);
}

#[test]
fn properties_follow_their_corners() {
    let mut input = cuboid(2, 3, 4);
    input.props = PropTable {
        width: 1,
        values: vec![0.5],
    };
    input.corner_prop = vec![NONE; 24];
    input.corner_prop[0] = 0;
    let p = mesh(&input).into_parts();
    assert_eq!(p.he_prop.iter().filter(|&&r| r == 0).count(), 1);
    assert_eq!(p.props.rows(), 1);
}

#[test]
fn an_empty_mesh_is_a_mesh() {
    let m = mesh(&soup(Vec::new(), &[]));
    assert_eq!(m.kind(), MeshKind::Closed);
    assert_eq!(six(&m), 0);
}

#[test]
fn morton_interleaves_the_shifted_bits() {
    let seed = resolve("geom-mesh exact morton", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let reach = NonZeroU64::new((1 << 41) - 1).unwrap();
    let edge = (1_i64 << 40) - 1;
    let mut points = vec![(0, 0, 0), (edge, -edge, edge), (-edge, edge, -edge)];
    points.extend((0..2000).map(|_| {
        (
            rng.below(reach) as i64 - edge,
            rng.below(reach) as i64 - edge,
            rng.below(reach) as i64 - edge,
        )
    }));
    for (x, y, z) in points {
        let shifted = [x, y, z].map(|c| (c + (1 << 40)) as u64);
        let mut want = 0_u128;
        for bit in 0..41 {
            for (axis, c) in shifted.iter().enumerate() {
                want |= u128::from((c >> bit) & 1) << (3 * bit + axis);
            }
        }
        assert_eq!(morton(P3::new(x, y, z).unwrap()), want, "({x}, {y}, {z})");
    }
}

#[test]
fn a_raw_mesh_checks_only_its_indices() {
    let v = vec![P3::new(0, 0, 0).unwrap(); 3];
    let raw = RawMesh::new(v.clone(), vec![0, 1, 2, 0, 0, 0], vec![0, 3, 6]).unwrap();
    assert_eq!(raw.vertices().len(), 3);
    assert_eq!(raw.corners().len(), 6);
    assert_eq!(raw.face_start(), [0, 3, 6]);
    assert_eq!(
        RawMesh::new(v.clone(), vec![0, 1, 3], vec![0, 3]),
        Err(MeshFault::Shape)
    );
    assert_eq!(
        RawMesh::new(v.clone(), vec![0, 1], vec![0, 2]),
        Err(MeshFault::ShortLoop { looped: 0 })
    );
    assert_eq!(
        RawMesh::new(v.clone(), vec![0, 1, 2], vec![1, 3]),
        Err(MeshFault::Shape)
    );
    assert_eq!(
        RawMesh::new(v, vec![0, 1, 2], vec![0, 2]),
        Err(MeshFault::Shape)
    );
}

#[test]
fn closed_height_fields_have_the_volume_of_their_columns() {
    let seed = resolve("geom-mesh exact prisms", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let below = |rng: &mut SplitMix64, n: u64| rng.below(NonZeroU64::new(n).unwrap());
    for _ in 0..64 {
        let n = 2 + below(&mut rng, 7) as usize;
        let step = 1 + below(&mut rng, 1000) as i64;
        let heights: Vec<Vec<i64>> = (0..n)
            .map(|_| (0..n).map(|_| 1 + below(&mut rng, 5000) as i64).collect())
            .collect();
        let (input, want) = prism(&heights, step);
        let m = mesh(&input);
        assert_eq!(m.kind(), MeshKind::Closed);
        assert_eq!(six(&m), want);
    }
}

#[test]
fn shuffled_input_gives_the_same_mesh() {
    let seed = resolve("geom-mesh exact shuffles", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let heights: Vec<Vec<i64>> = (0..6)
        .map(|y| (0..6).map(|x| 1 + (x * 7 + y * 13) % 11).collect())
        .collect();
    let (input, _) = prism(&heights, 16);
    let want = mesh(&input);
    let below = |rng: &mut SplitMix64, n: u64| rng.below(NonZeroU64::new(n).unwrap());
    for _ in 0..16 {
        let n = input.vertices.len();
        let mut order: Vec<u32> = (0..n as u32).collect();
        for i in (1..n).rev() {
            order.swap(i, below(&mut rng, i as u64 + 1) as usize);
        }
        let mut vertices = input.vertices.clone();
        for (v, &to) in order.iter().enumerate() {
            vertices[to as usize] = input.vertices[v];
        }
        let mut faces: Vec<Vec<u32>> = input
            .loop_start
            .windows(2)
            .map(|w| {
                let mut f: Vec<u32> = input.corners[w[0] as usize..w[1] as usize]
                    .iter()
                    .map(|&v| order[v as usize])
                    .collect();
                let turn = below(&mut rng, f.len() as u64) as usize;
                f.rotate_left(turn);
                f
            })
            .collect();
        for i in (1..faces.len()).rev() {
            faces.swap(i, below(&mut rng, i as u64 + 1) as usize);
        }
        let refs: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
        assert_eq!(mesh(&soup(vertices, &refs)), want);
    }
}

#[test]
fn the_checker_refuses_three_planes_that_do_not_make_a_new_point() {
    // A box's corner as three planes of its pool: on the grid, so in the wrong form.
    let on_grid = fault(|p| {
        let row = |n: [i128; 3], d: i128| {
            let wanted = Plane::new(n, d).unwrap();
            p.planes.iter().position(|q| *q == wanted).unwrap() as u32
        };
        let rows = [row([0, 0, 1], 0), row([0, 1, 0], 0), row([1, 0, 0], 0)];
        let mut rows = rows;
        rows.sort_unstable();
        p.verts.push(Vertex::Planes(rows));
    });
    assert!(matches!(
        on_grid,
        MeshError::Fault(MeshFault::ImplicitOnTheGrid { vertex: 8 })
    ));
    // Opposite faces of the box are parallel, so their planes meet in no point.
    let apart = fault(|p| {
        let row = |n: [i128; 3], d: i128| {
            let wanted = Plane::new(n, d).unwrap();
            p.planes.iter().position(|q| *q == wanted).unwrap() as u32
        };
        let mut rows = [row([1, 0, 0], 0), row([1, 0, 0], -2), row([0, 1, 0], 0)];
        rows.sort_unstable();
        p.verts.push(Vertex::Planes(rows));
    });
    assert!(matches!(
        apart,
        MeshError::Fault(MeshFault::PlanesDoNotMeet { vertex: 8 })
    ));
}

#[test]
fn one_point_made_two_ways_is_refused() {
    // The diagonal plane x = y also passes through the corner of the south and west sides, so this
    // spare vertex is that corner under other planes.
    let mut input = frustum();
    let Corner::Planes([top, s, w]) = input.vertices[4] else {
        panic!("the corner is implicit")
    };
    let diagonal = Plane::new([1, -1, 0], 0).unwrap();
    input.vertices.push(Corner::Planes([top, s, diagonal]));
    assert!(matches!(
        build(&input, &Exec::serial(), &mut meter()),
        Err(MeshError::Fault(MeshFault::RepeatedVertex { .. }))
    ));
    assert_ne!(w, diagonal);
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn a_pool_changes_nothing() {
    use geom_pool::native::Rayon;

    let heights: Vec<Vec<i64>> = (0..48)
        .map(|y| (0..48).map(|x| 1 + (x * 7 + y * 13) % 101).collect())
        .collect();
    let (input, _) = prism(&heights, 256);
    let want = mesh(&input);
    for threads in [2, 3, 8] {
        let rayon = Rayon::new(std::num::NonZeroUsize::new(threads).unwrap()).unwrap();
        let exec = Exec::with_pool(&rayon);
        assert_eq!(build(&input, &exec, &mut meter()).unwrap(), want);
        let mut broken = input.clone();
        broken.corners.swap(0, 1);
        assert_eq!(
            build(&broken, &exec, &mut meter()),
            build(&broken, &Exec::serial(), &mut meter())
        );
    }
}

/// A budget that runs out stops the build: as the meter's stop where a serial step meets it, or as
/// the phase over budget where the parallel helper does.
#[test]
fn the_budget_stops_a_build() {
    let input = cuboid(2, 3, 4);
    let mut small = WorkMeter::new(Budget::of_work(3));
    assert!(matches!(
        build(&input, &Exec::serial(), &mut small),
        Err(MeshError::Stopped(_) | MeshError::BudgetExceeded(_))
    ));
}

/// A face on a leaning plane, with a square hole, every corner off the grid. With `wrong`, the hole
/// goes round the way its face does, its edges' planes turned to suit.
fn holed(wrong: bool) -> Polygons {
    let top = Plane::new([-1, 0, 7], -1).unwrap();
    let wall = |n: [i128; 3], d: i128| Plane::new(n, d).unwrap();
    let (south, east) = (wall([0, 1, 0], 0), wall([-1, 0, 0], 7));
    let (north, west) = (wall([0, -1, 0], 7), wall([1, 0, 0], 0));
    let (left, far) = (wall([-1, 0, 0], 2), wall([0, 1, 0], -4));
    let (right, near) = (wall([1, 0, 0], -4), wall([0, -1, 0], 2));
    let vertices = vec![
        Corner::Planes([top, south, west]),
        Corner::Planes([top, south, east]),
        Corner::Planes([top, north, east]),
        Corner::Planes([top, north, west]),
        Corner::Planes([top, near, left]),
        Corner::Planes([top, far, left]),
        Corner::Planes([top, far, right]),
        Corner::Planes([top, near, right]),
    ];
    let mut input = soup(vertices, &[&[0, 1, 2, 3]]);
    let turn = |p: Plane| Plane::new(p.n().map(|c| -c), -p.d()).unwrap();
    let mut planes = vec![south, east, north, west];
    if wrong {
        input.corners.extend([4, 7, 6, 5]);
        planes.extend([turn(near), turn(right), turn(far), turn(left)]);
    } else {
        input.corners.extend([4, 5, 6, 7]);
        planes.extend([left, far, right, near]);
    }
    input.loop_start.push(8);
    input.face_loops = vec![0, 2];
    input.face_plane = vec![Some(top)];
    input.edge_plane = planes.into_iter().map(Some).collect();
    input
}

#[test]
fn a_face_off_the_grid_turns_the_way_it_faces() {
    let parts = mesh(&slab(1 << 10)).into_parts();
    let refused = |p: Parts| ExactMesh::new(p, &Exec::serial(), &mut meter()).unwrap_err();
    // The face alone turned over: its edges no longer run the way their planes say.
    let mut one = parts.clone();
    one.face_plane[0].flipped ^= true;
    assert!(matches!(
        refused(one),
        MeshError::Fault(MeshFault::EdgePlaneFacing { .. })
    ));
    // One edge's plane turned over.
    let mut edge = parts.clone();
    edge.he_plane[0].flipped ^= true;
    assert!(matches!(
        refused(edge),
        MeshError::Fault(MeshFault::EdgePlaneFacing { half_edge: 0 })
    ));
    // The face and all its edges together: every line runs as it did, and only the turn tells.
    let mut all = parts.clone();
    all.face_plane[0].flipped ^= true;
    let end = all.loop_start[1] as usize;
    for facing in &mut all.he_plane[..end] {
        facing.flipped ^= true;
    }
    assert!(matches!(
        refused(all),
        MeshError::Fault(MeshFault::WrongWayRound { face: 0 })
    ));
}

/// One flat dart on a leaning plane, counter-clockwise from above, its corners in `grid` on the grid
/// and the others off it. The fourth corner sits in its notch, so it turns the other way.
fn dart(outline: [(i128, i128); 4], grid: &[usize]) -> Polygons {
    let top = Plane::new([-1, 0, 7], -1).unwrap();
    let wall = |k: usize| {
        let (px, py) = outline[k];
        let (qx, qy) = outline[(k + 1) % 4];
        let n = [py - qy, qx - px, 0];
        Plane::new(n, -(n[0] * px + n[1] * py)).unwrap()
    };
    let vertices = (0..4)
        .map(|k| {
            if grid.contains(&k) {
                let (x, y) = outline[k];
                g(x as i64, y as i64, ((x + 1) / 7) as i64)
            } else {
                Corner::Planes([top, wall((k + 3) % 4), wall(k)])
            }
        })
        .collect();
    let mut input = soup(vertices, &[&[0, 1, 2, 3]]);
    input.face_plane = vec![Some(top)];
    input.edge_plane = (0..4).map(|k| Some(wall(k))).collect();
    input
}

/// A dart's face and all its edges turned over together, which only the turn at its lowest corner
/// tells: the notch would turn the way the face now wants.
fn turned_over(m: ExactMesh) -> MeshError {
    let mut parts = m.into_parts();
    parts.face_plane[0].flipped ^= true;
    for facing in &mut parts.he_plane {
        facing.flipped ^= true;
    }
    ExactMesh::new(parts, &Exec::serial(), &mut meter()).unwrap_err()
}

#[test]
fn a_notch_off_the_grid_is_no_wrong_turn() {
    let shape: [(i128, i128); 4] = [(0, -7), (14, 0), (0, 7), (5, 0)];
    // Each turn and mirror puts the notch somewhere else in the loop's stored order.
    for turns in 0..4 {
        let turn = |mut p: (i128, i128)| {
            for _ in 0..turns {
                p = (-p.1, p.0);
            }
            p
        };
        let [a, b, c, d] = shape.map(turn);
        let [e, f, h, i] = shape.map(|(x, y)| turn((x, -y)));
        for outline in [[a, b, c, d], [i, h, f, e]] {
            let m = mesh(&dart(outline, &[]));
            assert!(matches!(
                turned_over(m),
                MeshError::Fault(MeshFault::WrongWayRound { face: 0 })
            ));
        }
    }
}

#[test]
fn corners_on_and_off_the_grid_sort_together() {
    let m = mesh(&dart([(-1, -7), (13, 0), (-1, 7), (5, 0)], &[0, 1, 2]));
    assert!(matches!(
        turned_over(m),
        MeshError::Fault(MeshFault::WrongWayRound { face: 0 })
    ));
}

/// A dart whose loop is stored from its notch: the notch's walls reduce to normals (2, ±1, 0), and
/// sort just after the face's plane, ahead of the long walls (6, ±11, 0), so the notch has the lowest
/// key and the loop starts there. The checker must still find the corner lowest in x, y and z.
/// The work `build` charges is part of what it does: a budget stops it at the same item every
/// time (23.10). One unit per input vertex, per corner, per edge and twice per face, and the
/// checker's own, pinned for two shapes; a change to the schedule changes these numbers.
#[test]
fn the_build_charges_its_light_phases_too() {
    let charged = |input: &Polygons| {
        let mut meter = meter();
        build(input, &Exec::serial(), &mut meter).unwrap();
        meter.used().get(BudgetKind::Work)
    };
    assert_eq!(charged(&cuboid(3, 4, 5)), 94);
    let field = prism(&[vec![5, 6, 7], vec![6, 8, 6], vec![5, 7, 9]], 4).0;
    assert_eq!(charged(&field), 313);
}

/// Two faces on one dominant axis share their edge's plane only where `build` makes it: an edge
/// whose planes are given keeps each half-edge's own, even where the two differ.
#[test]
fn given_edge_planes_are_never_shared() {
    // One cell of a height field: two top triangles, a b c and a c d, meet on the diagonal a c.
    let (mut input, _) = prism(&[vec![5, 6], vec![7, 6]], 4);
    let corner = |k: usize| match input.vertices[input.corners[k] as usize] {
        Corner::Grid(p) => p,
        Corner::Planes(_) => panic!("a prism's corners are grid points"),
    };
    let (a, c) = (corner(0), corner(2));
    assert_eq!((corner(3), corner(4)), (a, c));
    // The diagonal's half-edges: c to a in the first triangle, a to c in the second.
    let (b, d) = (corner(1), corner(5));
    let tilted = |start: P3, off: [i64; 3], inside: P3| {
        let lift = g(
            start.x.get() + off[0],
            start.y.get() + off[1],
            start.z.get() + off[2],
        );
        let Corner::Grid(lift) = lift else {
            panic!("a grid point")
        };
        let plane = Plane::through(a, c, lift).unwrap();
        // Positive toward the face's own third corner.
        let [x, y, z] = plane.n();
        let at = |p: P3| {
            x * i128::from(p.x.get())
                + y * i128::from(p.y.get())
                + z * i128::from(p.z.get())
                + plane.d()
        };
        if at(inside) > 0 {
            plane
        } else {
            Plane::new([-x, -y, -z], -plane.d()).unwrap()
        }
    };
    let first = tilted(a, [1, -2, 3], b);
    let second = tilted(a, [-2, 1, 5], d);
    let mut given = vec![None; input.corners.len()];
    given[2] = Some(first);
    given[3] = Some(second);
    input.edge_plane = given;
    let mesh = build(&input, &Exec::serial(), &mut meter()).unwrap();
    let pooled = |plane: &Plane| {
        let [x, y, z] = plane.n();
        let turned = Plane::new([-x, -y, -z], -plane.d()).unwrap();
        mesh.parts()
            .planes
            .iter()
            .any(|q| q == plane || *q == turned)
    };
    assert!(pooled(&first) && pooled(&second));
}

#[test]
fn a_loop_stored_from_its_notch_turns_at_its_lowest_corner() {
    let outline = [(0, -6), (11, 0), (0, 6), (3, 0)];
    let m = mesh(&dart(outline, &[]));
    let parts = m.parts();
    let first = parts.verts[parts.he_vert[parts.loop_start[0] as usize] as usize];
    let Vertex::Planes(rows) = first else {
        panic!("the dart is off the grid");
    };
    let notch: Vec<Plane> = [(0, 6), (3, 0), (0, -6)]
        .windows(2)
        .map(|w| {
            let ((px, py), (qx, qy)) = (w[0], w[1]);
            let n = [py - qy, qx - px, 0];
            Plane::new(n, -(n[0] * px + n[1] * py)).unwrap()
        })
        .collect();
    let [a, b, c] = rows.map(|r| parts.planes[r as usize]);
    let point = Tpi::new(a, b, c).unwrap();
    for wall in &notch {
        assert_eq!(wall.side_of_tpi(&point).unwrap(), Sign::Zero);
    }
    assert!(matches!(
        turned_over(m),
        MeshError::Fault(MeshFault::WrongWayRound { face: 0 })
    ));
}

#[test]
fn a_hole_off_the_grid_goes_the_other_way_round() {
    let m = mesh(&holed(false));
    assert_eq!(m.parts().loop_start.len(), 3);
    assert!(matches!(
        build(&holed(true), &Exec::serial(), &mut meter()),
        Err(MeshError::Fault(MeshFault::WrongWayRound { face: 0 }))
    ));
}

#[test]
fn a_loop_no_face_holds_is_refused() {
    let mut input = cuboid(2, 3, 4);
    input.corners.extend([0, 1, 2]);
    input.loop_start.push(input.corners.len() as u32);
    assert_eq!(
        build(&input, &Exec::serial(), &mut meter()).unwrap_err(),
        MeshError::Fault(MeshFault::Shape)
    );
}

#[test]
fn planes_of_any_size_sort_exactly() {
    // Offsets of 1.75 · 2^61 and 1.86 · 2^60: the smaller has the higher top bits. Each slab's two
    // offsets differ by 1 and share their top 56 bits, so only the whole values can order them.
    let mut input = join(slab(1 << 39), &slab(17 << 34));
    let mut rng = SplitMix64::new(resolve("geom-mesh exact big planes", Seed(0x146)).unwrap());
    let reach = NonZeroU64::new(1 << 39).unwrap();
    for _ in 0..24 {
        input = join(input, &slab(rng.below(reach) as i64 + (1 << 33)));
    }
    let m = mesh(&input);
    assert_eq!(m.faces(), 26 * 6);
    // Each slab's two offsets, its sides, and the two planes every slab shares.
    assert_eq!(m.parts().planes.len(), 26 * 4 + 2);
}

#[test]
fn scaling_a_solid_leaves_its_normals_alone() {
    let tetrahedron = |s: i64| {
        soup(
            vec![g(0, 0, 0), g(3 * s, 0, 0), g(0, 5 * s, 0), g(0, 0, 7 * s)],
            &[&[0, 2, 1], &[0, 1, 3], &[1, 2, 3], &[2, 0, 3]],
        )
    };
    let normals = |input: &Polygons| -> Vec<[i128; 3]> {
        mesh(input).parts().planes.iter().map(Plane::n).collect()
    };
    assert_eq!(normals(&tetrahedron(1)), normals(&tetrahedron(1 << 20)));
}

#[test]
fn faces_tied_on_their_start_settle_by_the_rest() {
    let mut vertices = cuboid(4, 4, 4).vertices;
    vertices.push(g(4, 2, 4));
    let sides: [&[u32]; 5] = [
        &[0, 3, 2, 1],
        &[0, 1, 5, 4],
        &[1, 2, 6, 8, 5],
        &[2, 3, 7, 6],
        &[3, 0, 4, 7],
    ];
    // Round the top from its lowest corner, which starts each of these with the same three
    // vertices but for the last two faces' order.
    let fan = |top: [&'static [u32]; 3]| {
        let faces: Vec<&[u32]> = sides.iter().copied().chain(top).collect();
        mesh(&soup(vertices.clone(), &faces)).into_parts()
    };
    let a: &[u32] = &[4, 5, 8];
    let (b, c): (&[u32], &[u32]) = (&[4, 8, 6], &[4, 6, 7]);
    assert_eq!(fan([a, b, c]), fan([c, b, a]));
}

#[test]
fn a_loop_through_its_lowest_vertex_twice_starts_at_the_first_visit() {
    let m = mesh(&soup(
        vec![g(0, 0, 0), g(4, 1, 0), g(4, 2, 0), g(2, 4, 0), g(1, 4, 0)],
        &[&[0, 1, 2, 0, 3, 4]],
    ));
    let p = m.parts();
    assert_eq!(p.he_vert[0], p.he_vert[3]);
    assert_eq!(
        p.verts[p.he_vert[1] as usize],
        Vertex::Grid(P3::new(4, 1, 0).unwrap())
    );
}

#[test]
fn the_rows_of_a_vertex_must_rise() {
    let mut p = mesh(&frustum()).into_parts();
    let Some(Vertex::Planes(rows)) = p.verts.last_mut() else {
        panic!("the frustum's last vertex is off the grid");
    };
    rows.swap(1, 2);
    assert!(matches!(
        ExactMesh::new(p, &Exec::serial(), &mut meter()),
        Err(MeshError::Fault(MeshFault::VerticesOutOfOrder { .. }))
    ));
}

#[test]
fn errors_say_what_went_wrong() {
    assert_ne!(MeshFault::Shape.to_string(), "");
    let texts = [
        MeshError::Fault(MeshFault::Shape).to_string(),
        MeshError::FacePlaneNeeded { face: 1 }.to_string(),
        MeshError::EdgePlaneNeeded { corner: 1 }.to_string(),
        MeshError::Full.to_string(),
        MeshError::Num(NumError::Degenerate).to_string(),
    ];
    assert!(texts.iter().all(|t| !t.is_empty()));
    let _ = I256::from_i128(0);
}
