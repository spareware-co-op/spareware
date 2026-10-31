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
use geom_num::plane::Plane;
use geom_num::wide::I256;
use geom_num::{Exactness, NumError};
use geom_op::NameId;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

mod shapes;

use shapes::{cuboid, frustum, g, prism, soup, source};

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
    // Every pair is the other box's face or its own: each pair bounds one solid.
    let p = m.parts();
    for (h, &q) in p.he_pair.iter().enumerate() {
        assert_eq!(p.he_pair[q as usize] as usize, h);
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

#[test]
fn the_budget_stops_a_build() {
    let input = cuboid(2, 3, 4);
    let mut small = WorkMeter::new(Budget::of_work(3));
    assert!(matches!(
        build(&input, &Exec::serial(), &mut small),
        Err(MeshError::Stopped(_))
    ));
}

#[test]
fn errors_say_what_went_wrong() {
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
