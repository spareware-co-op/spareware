//! The second checker on exact meshes (chapter 28.4; piece G135): it agrees with `geom-mesh`'s own
//! on every valid mesh and every broken copy of one, and with its volumes.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "test code: a failure should stop loudly"
)]

use dashu_int::IBig;
use dashu_ratio::RBig;
use geom_mesh::exact::build::{Corner, Polygons, Source, build};
use geom_mesh::exact::mass::Volume;
use geom_mesh::exact::{ExactMesh, MeshKind, PropTable, Role, Vertex};
use geom_num::Exactness;
use geom_num::grid::{P3, Steps};
use geom_num::plane::Plane;
use geom_op::NameId;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::second_mesh::{Finding, check, six_volume};

fn g(x: i64, y: i64, z: i64) -> Corner {
    Corner::Grid(P3::new(x, y, z).unwrap())
}

/// Polygons of one run, each face one loop.
fn soup(vertices: Vec<Corner>, faces: &[&[u32]]) -> Polygons {
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

fn cuboid(a: i64, b: i64, c: i64) -> Polygons {
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

/// A box of side 30 with a corner cut by 3x + 4y + 7z = 370, whose new corners are off the grid.
fn cut_box() -> Polygons {
    let plane = |a: [i64; 3], b: [i64; 3], c: [i64; 3]| {
        let p = |q: [i64; 3]| P3::new(q[0], q[1], q[2]).unwrap();
        Plane::through(p(a), p(b), p(c)).unwrap()
    };
    let hx = plane([30, 0, 0], [30, 30, 0], [30, 0, 30]);
    let hy = plane([0, 30, 0], [0, 30, 30], [30, 30, 0]);
    let hz = plane([0, 0, 30], [30, 0, 30], [0, 30, 30]);
    let cut = plane([20, 25, 30], [24, 22, 30], [26, 24, 28]);
    let lz = plane([0, 0, 0], [0, 30, 0], [30, 0, 0]);
    let ly = plane([0, 0, 0], [30, 0, 0], [0, 0, 30]);
    let lx = plane([0, 0, 0], [0, 0, 30], [0, 30, 0]);
    let vertices = vec![
        g(0, 0, 0),
        g(30, 0, 0),
        g(0, 30, 0),
        g(30, 30, 0),
        g(0, 0, 30),
        g(30, 0, 30),
        g(0, 30, 30),
        Corner::Planes([hx, hy, cut]),
        Corner::Planes([hx, hz, cut]),
        Corner::Planes([hy, hz, cut]),
    ];
    let mut out = soup(
        vertices,
        &[
            &[0, 2, 3, 1],
            &[0, 1, 5, 4],
            &[0, 4, 6, 2],
            &[1, 3, 7, 8, 5],
            &[2, 6, 9, 7, 3],
            &[4, 5, 8, 9, 6],
            &[7, 9, 8],
        ],
    );
    out.face_plane = [lz, ly, lx, hx, hy, hz, cut].map(Some).to_vec();
    // Each edge plane faces into its face: a neighbour's plane, turned.
    let inward = |p: Plane| Some(Plane::new(p.n().map(|c| -c), -p.d()).unwrap());
    out.edge_plane = vec![
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        // hx: 1 3 7 8 5.
        None,
        inward(hy),
        inward(cut),
        inward(hz),
        None,
        // hy: 2 6 9 7 3.
        None,
        inward(hz),
        inward(cut),
        inward(hx),
        None,
        // hz: 4 5 8 9 6.
        None,
        inward(hx),
        inward(cut),
        inward(hy),
        None,
        // cut: 7 9 8.
        inward(hy),
        inward(hz),
        inward(hx),
    ];
    out
}

/// Two boxes of side 10 touching along the edge from (10, 10, 0) to (10, 10, 10).
fn touching() -> Polygons {
    let at = |x: i64, y: i64, z: i64| {
        [
            [x, y, z],
            [x + 10, y, z],
            [x + 10, y + 10, z],
            [x, y + 10, z],
            [x, y, z + 10],
            [x + 10, y, z + 10],
            [x + 10, y + 10, z + 10],
            [x, y + 10, z + 10],
        ]
    };
    let mut rows: Vec<[i64; 3]> = at(0, 0, 0).to_vec();
    let mut second = [0_u32; 8];
    for (slot, p) in second.iter_mut().zip(at(10, 10, 0)) {
        *slot = rows.iter().position(|q| *q == p).unwrap_or_else(|| {
            rows.push(p);
            rows.len() - 1
        }) as u32;
    }
    let rings = |v: [u32; 8]| {
        [
            [v[0], v[3], v[2], v[1]],
            [v[4], v[5], v[6], v[7]],
            [v[0], v[1], v[5], v[4]],
            [v[1], v[2], v[6], v[5]],
            [v[2], v[3], v[7], v[6]],
            [v[3], v[0], v[4], v[7]],
        ]
    };
    let faces: Vec<[u32; 4]> = rings([0, 1, 2, 3, 4, 5, 6, 7])
        .into_iter()
        .chain(rings(second))
        .collect();
    let lists: Vec<&[u32]> = faces.iter().map(<[u32; 4]>::as_slice).collect();
    soup(rows.iter().map(|p| g(p[0], p[1], p[2])).collect(), &lists)
}

fn mesh(input: &Polygons) -> ExactMesh {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    build(input, &Exec::serial(), &mut meter).unwrap()
}

/// What `geom-mesh`'s checker says of `parts`.
fn first_checks(parts: geom_mesh::exact::Parts) -> bool {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    ExactMesh::new(parts, &Exec::serial(), &mut meter).is_ok()
}

fn corpus() -> Vec<ExactMesh> {
    vec![
        mesh(&cuboid(10, 20, 30)),
        mesh(&cuboid(1, 1, 1)),
        mesh(&cuboid(1_000_000, 3, 7)),
        mesh(&cut_box()),
        mesh(&touching()),
    ]
}

#[test]
fn every_valid_mesh_passes_both() {
    for m in corpus() {
        assert_eq!(check(m.parts()), []);
        assert!(first_checks(m.parts().clone()));
    }
}

#[test]
fn the_volumes_agree() {
    let m = mesh(&cuboid(10, 20, 30));
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let Volume::Exact(exact) = m.volume(&mut meter).unwrap() else {
        panic!("a mesh of grid points");
    };
    assert_eq!(exact, geom_num::wide::I256::from_i64(36_000));
    assert_eq!(
        six_volume(m.parts()),
        Some(RBig::from(IBig::from(36_000_i64)))
    );
    let m = mesh(&cut_box());
    let Volume::Certified(six) = m.volume(&mut meter).unwrap() else {
        panic!("a mesh of meetings of planes");
    };
    let truth = six_volume(m.parts()).unwrap();
    let (lo, hi) = (
        RBig::try_from(six.lo()).unwrap(),
        RBig::try_from(six.hi()).unwrap(),
    );
    assert!(lo <= truth && truth <= hi);
}

/// A broken copy of each mesh, each breaking one rule.
fn breaks() -> Vec<(&'static str, fn(&mut geom_mesh::exact::Parts))> {
    vec![
        ("a plane moved", |p| {
            let plane = p.planes[0];
            p.planes[0] = Plane::new(plane.n(), plane.d() + 1).unwrap();
        }),
        ("a half-edge's vertex changed", |p| {
            p.he_vert[0] = p.he_vert[1];
        }),
        ("a face turned", |p| {
            p.face_plane[0].flipped = !p.face_plane[0].flipped;
        }),
        ("an edge plane moved to the face's", |p| {
            p.he_plane[0] = p.face_plane[1];
        }),
        ("a pair cut", |p| {
            p.he_pair[0] = p.he_pair[1];
        }),
        ("the wrong kind", |p| {
            p.kind = if p.kind == MeshKind::General {
                MeshKind::Closed
            } else {
                MeshKind::General
            };
        }),
        ("the arrays short", |p| {
            p.he_pair.pop();
        }),
        ("a loop start moved", |p| {
            p.loop_start[1] = 1;
        }),
        ("an edge plane turned", |p| {
            p.he_plane[0].flipped = !p.he_plane[0].flipped;
        }),
        ("a deviation past the class", |p| {
            p.face_dev[0] = Steps(1);
        }),
    ]
}

#[test]
fn every_broken_mesh_is_refused_by_both() {
    for m in corpus() {
        for (name, brk) in breaks() {
            let mut parts = m.parts().clone();
            brk(&mut parts);
            let second = check(&parts);
            assert_ne!(second, [], "{name}: the second checker");
            assert!(!first_checks(parts), "{name}: the first checker");
        }
    }
}

#[test]
fn pairs_across_the_wrong_cell_are_refused_by_both() {
    let m = mesh(&touching());
    assert_eq!(m.parts().kind, MeshKind::General);
    let mut parts = m.parts().clone();
    // The four half-edges of the shared edge, by its two vertices.
    let shared = |p: [i64; 3]| {
        let at = P3::new(p[0], p[1], p[2]).unwrap();
        parts
            .verts
            .iter()
            .position(|v| *v == Vertex::Grid(at))
            .unwrap() as u32
    };
    let (low, high) = (shared([10, 10, 0]), shared([10, 10, 10]));
    let ends: Vec<usize> = (0..parts.he_vert.len())
        .filter(|&h| {
            let pair = parts.he_pair[h] as usize;
            let (a, b) = (parts.he_vert[h], parts.he_vert[pair]);
            (a, b) == (low, high)
        })
        .collect();
    assert_eq!(ends.len(), 2);
    let (x, y) = (ends[0], ends[1]);
    let (px, py) = (parts.he_pair[x], parts.he_pair[y]);
    parts.he_pair[x] = py;
    parts.he_pair[py as usize] = x as u32;
    parts.he_pair[y] = px;
    parts.he_pair[px as usize] = y as u32;
    assert!(
        check(&parts)
            .iter()
            .all(|f| matches!(f, Finding::BadPair(_)))
    );
    assert_ne!(check(&parts), []);
    assert!(!first_checks(parts));
}

#[test]
fn a_meeting_on_the_grid_and_a_flip_off_it_are_findings() {
    let m = mesh(&cut_box());
    // The cut's face has only corners off the grid, so only the second checker turns it.
    let mut parts = m.parts().clone();
    let face = (0..parts.face_plane.len())
        .find(|f| {
            let plane = parts.planes[parts.face_plane[*f].row as usize];
            plane.n() == [3, 4, 7]
        })
        .unwrap();
    parts.face_plane[face].flipped = !parts.face_plane[face].flipped;
    assert!(
        check(&parts)
            .iter()
            .any(|f| matches!(f, Finding::WrongWay(_)))
    );
}

#[test]
fn a_meeting_that_does_not_meet_is_a_finding() {
    let m = mesh(&cut_box());
    let mut parts = m.parts().clone();
    let row = parts
        .verts
        .iter()
        .position(|v| matches!(v, Vertex::Planes(_)))
        .unwrap();
    let Vertex::Planes([a, b, _]) = parts.verts[row] else {
        panic!("a meeting");
    };
    parts.verts[row] = Vertex::Planes([a, b, b]);
    assert!(matches!(check(&parts).as_slice(), [Finding::NoPoint(_)]));
    assert!(!first_checks(parts));
}

#[test]
fn the_second_checker_shares_no_code_with_the_first() {
    let source = include_str!("../src/second_mesh.rs");
    for name in [
        "exact::check",
        "morton",
        "mass::",
        "side_of_tpi",
        "same_point",
    ] {
        assert!(!source.contains(name), "{name}");
    }
}
