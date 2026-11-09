//! Healing imported meshes through the public interface (piece G298), against what 19.7 says of
//! each step: welds within ε to a representative that never moves, which closes the holes within
//! ε, cracks stitched without a move, wider holes refused with where they are, faces turned outward
//! by exact volume, overlaps resolved by winding number, and the class what moved.

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

#[cfg(not(target_family = "wasm"))]
use std::num::NonZeroUsize;

use geom_mesh::exact::build::Source;
use geom_mesh::exact::mass::Volume;
use geom_mesh::exact::raw::RawMesh;
use geom_mesh::exact::{ExactMesh, MeshError, MeshKind, Role, morton};
use geom_mesh::repair::weld::{Removal, Removed};
use geom_mesh::repair::{Boundary, Heal, HealError, Healed, heal, quantise, weld};
use geom_num::Exactness;
use geom_num::grid::{GridStep, P3, Steps};
use geom_num::wide::I256;
use geom_op::{Bound, NameId, OpId, OpPath, Remedy, Value};
#[cfg(not(target_family = "wasm"))]
use geom_pool::native::Rayon;
use geom_run::{Budget, Exec, WorkMeter};

/// One millimetre on `Q0`'s grid.
const MM: i64 = 6400;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 26))
}

fn source() -> Source {
    Source {
        name: NameId::from_bytes([3; 16]),
        role: Role::FaceSet,
        flags: 0,
    }
}

fn step() -> OpPath {
    OpPath::new(OpId::from_bytes([9; 16]))
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

fn heal_at(mesh: &RawMesh, epsilon: u64) -> Result<Healed, HealError> {
    heal(
        mesh,
        Heal {
            epsilon: Steps(epsilon),
            step: GridStep::Q0,
        },
        source(),
        &step(),
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

fn closed(h: &Healed) {
    assert!(h.mesh.is_solid());
    assert_ne!(h.mesh.kind(), MeshKind::Open);
}

/// A cube of side `side` as a soup: each face has its own corners. The first face to use a corner
/// has it exactly; each later face's copy is nudged up by `nudge(face)` on every axis, so the exact
/// copy has the lowest key and is the representative.
fn soup_cube(side: i64, nudge: impl Fn(u32) -> [i64; 3]) -> RawMesh {
    let box_corners = corners([0; 3], [side; 3]);
    let mut seen = [false; 8];
    let mut points = Vec::new();
    let mut faces = Vec::new();
    for (f, face) in (0_u32..).zip(sides(0)) {
        let mut looped = Vec::new();
        for v in face {
            let [x, y, z] = box_corners[v as usize];
            let [dx, dy, dz] = if seen[v as usize] { nudge(f) } else { [0; 3] };
            seen[v as usize] = true;
            looped.push(points.len() as u32);
            points.push([x + dx, y + dy, z + dz]);
        }
        faces.push(looped);
    }
    raw(&points, &faces)
}

#[test]
fn a_cube_of_separate_faces_welds_into_the_cube() {
    let nudge = |f: u32| [17 * i64::from(f), 11 * i64::from(f), 7 * i64::from(f)];
    let h = heal_at(&soup_cube(MM, nudge), 640).unwrap();
    closed(&h);
    assert_eq!(six_volume(&h.mesh), cube_volume(MM));
    assert_eq!(h.mesh.faces(), 6);
    // The farthest copy is face 5's: (85, 55, 35), 107.6 steps.
    assert_eq!(h.log.weld.movement, Steps(108));
    assert_eq!(
        h.mesh.parts().class,
        Exactness::Certified {
            bound: Steps(108),
            step: GridStep::Q0
        }
    );
    // Every copy that moved went to its exact corner, and nothing was removed or split.
    assert_eq!(h.log.weld.joined.len(), 16);
    for j in &h.log.weld.joined {
        assert!(
            [j.to.x.get(), j.to.y.get(), j.to.z.get()]
                .iter()
                .all(|&c| c == 0 || c == MM)
        );
    }
    assert!(h.log.weld.removed.is_empty() && h.log.weld.split.is_empty());
}

#[test]
fn equal_points_join_without_a_move_and_the_result_is_exact() {
    let h = heal_at(&soup_cube(MM, |_| [0; 3]), 640).unwrap();
    assert_eq!(h.mesh.parts().class, Exactness::Exact);
    assert!(h.log.weld.joined.is_empty());
    assert_eq!(h.log.weld.movement, Steps(0));
    assert_eq!(six_volume(&h.mesh), cube_volume(MM));
}

#[test]
fn a_weld_distance_of_zero_joins_only_equal_points() {
    let w = weld(
        &soup_cube(MM, |_| [1, 0, 0]),
        Steps(0),
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    // Eight exact corners, and one step off each the one point its two later copies share.
    assert_eq!(w.mesh.vertices().len(), 8 + 8);
    assert!(w.log.joined.is_empty());
    let w = weld(
        &soup_cube(MM, |_| [1, 0, 0]),
        Steps(1),
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    assert_eq!(w.mesh.vertices().len(), 8);
    assert_eq!(w.log.joined.len(), 8);
    assert_eq!(w.log.movement, Steps(1));
    // The first two faces use only exact corners; each later one has copies that moved.
    assert_eq!(w.moved, [false, false, true, true, true, true]);
}

#[test]
fn representatives_never_move_so_a_chain_of_close_points_does_not_collapse() {
    // Four points 600 steps apart on a line, ε = 640: the first is a representative, the second
    // joins it, the third is 1,200 from the first, so it is a representative of its own, and the
    // fourth joins the third, not the second, which is no representative.
    let line: Vec<[i64; 3]> = (0..4).map(|i| [600 * i, 0, 0]).collect();
    let mut points = line.clone();
    points.extend([[0, MM, 0], [0, 0, MM]]);
    let faces = vec![vec![0, 1, 4], vec![2, 3, 5]];
    let w = weld(
        &raw(&points, &faces),
        Steps(640),
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    let joined: Vec<(i64, i64)> = w
        .log
        .joined
        .iter()
        .map(|j| (j.from.x.get(), j.to.x.get()))
        .collect();
    assert_eq!(joined, [(600, 0), (1800, 1200)]);
    assert_eq!(w.log.movement, Steps(600));
    // Each triangle lost a corner to the weld: two corners on one point leave no area.
    assert_eq!(
        w.log.removed,
        [
            Removed {
                face: 0,
                why: Removal::Degenerate
            },
            Removed {
                face: 1,
                why: Removal::Degenerate
            }
        ]
    );
}

#[test]
fn faces_the_wrong_way_round_are_turned_and_listed() {
    let mut faces = sides(0);
    faces[1].reverse();
    faces[4].reverse();
    let h = heal_at(&raw(&corners([0; 3], [MM; 3]), &faces), 640).unwrap();
    assert_eq!(h.log.orient.turned, [1, 4]);
    assert_eq!(six_volume(&h.mesh), cube_volume(MM));
    assert_eq!(h.mesh.parts().class, Exactness::Exact);
}

#[test]
fn an_inside_out_cube_is_turned_outward_by_its_volume() {
    let faces: Vec<Vec<u32>> = sides(0)
        .into_iter()
        .map(|mut f| {
            f.reverse();
            f
        })
        .collect();
    let h = heal_at(&raw(&corners([0; 3], [MM; 3]), &faces), 640).unwrap();
    assert_eq!(h.log.orient.turned, [0, 1, 2, 3, 4, 5]);
    assert_eq!(h.log.orient.patches, 1);
    assert_eq!(six_volume(&h.mesh), cube_volume(MM));
}

#[test]
fn two_cubes_on_one_edge_are_each_turned_outward() {
    // They share only the edge x = y = side; one is inside out. Each closes on its own, so each
    // is turned by its own volume, although that edge has four faces.
    let mut points = corners([0; 3], [MM; 3]);
    points.extend(corners([MM, MM, 0], [2 * MM, 2 * MM, MM]));
    let mut faces = sides(0);
    faces.extend(sides(8).into_iter().map(|mut f| {
        f.reverse();
        f
    }));
    let h = heal_at(&raw(&points, &faces), 640).unwrap();
    assert_eq!(h.log.orient.patches, 2);
    assert_eq!(h.log.orient.turned, [6, 7, 8, 9, 10, 11]);
    assert_eq!(
        six_volume(&h.mesh),
        cube_volume(MM).checked_add(&cube_volume(MM)).unwrap()
    );
}

/// A cube whose top face is a pentagon, the triangle of legs `leg` at the corner (0, 0, side) cut
/// off, so its sides' top edges hold the pentagon's two new corners: a crack, then a hole.
fn cube_with_a_corner_hole(side: i64, leg: i64) -> RawMesh {
    let mut points = corners([0; 3], [side; 3]);
    points.extend([[leg, 0, side], [0, leg, side]]);
    let mut faces = sides(0);
    faces[5] = vec![8, 5, 7, 6, 9];
    raw(&points, &faces)
}

#[test]
fn a_hole_within_epsilon_is_closed_by_welding_its_corners() {
    // The hole's corners are 300 steps from the cube's: the weld joins them to it, and the
    // pentagon becomes the whole top.
    let h = heal_at(&cube_with_a_corner_hole(MM, 300), 640).unwrap();
    closed(&h);
    assert_eq!(six_volume(&h.mesh), cube_volume(MM));
    let joined: Vec<(P3, P3)> = h.log.weld.joined.iter().map(|j| (j.from, j.to)).collect();
    assert_eq!(
        joined,
        [(p(300, 0, MM), p(0, 0, MM)), (p(0, 300, MM), p(0, 0, MM))]
    );
    assert!(h.log.holes.stitched.is_empty());
    assert_eq!(
        h.mesh.parts().class,
        Exactness::Certified {
            bound: Steps(300),
            step: GridStep::Q0
        }
    );
    // Every face carries the bound, none more.
    assert!(h.mesh.parts().face_dev.iter().all(|&d| d == Steps(300)));
}

#[test]
fn a_hole_wider_than_epsilon_is_left_open_and_listed() {
    let e = heal_at(&cube_with_a_corner_hole(MM, 1000), 640).unwrap_err();
    let open = Boundary {
        at: p(0, 0, MM),
        corners: 3,
        width: Steps(1415),
    };
    assert_eq!(
        e,
        HealError::NotClosed {
            boundaries: vec![open]
        }
    );
    let remedies = e.remedies(GridStep::Q0);
    assert!(remedies.contains(&Remedy::ChangeInput {
        input: "epsilon",
        to: Some((
            Bound::AtLeast,
            Value::Length {
                steps: Steps(1415),
                step: GridStep::Q0
            }
        ))
    }));
    assert!(remedies.contains(&Remedy::Accept {
        class: Exactness::Certified {
            bound: Steps(1415),
            step: GridStep::Q0
        }
    }));
    assert!(remedies.contains(&Remedy::UseRoute {
        route: "reconstruct"
    }));
    assert!(e.to_string().contains("1 open boundaries"));
    // The same hole closes at an ε as wide as it.
    let h = heal_at(&cube_with_a_corner_hole(MM, 1000), 1415).unwrap();
    assert_eq!(six_volume(&h.mesh), cube_volume(MM));
}

#[test]
fn a_hole_wider_than_the_cap_exceeds_the_tolerance() {
    // Its corners are 7,000 steps apart, too far to weld at the cap; it is 9,899.5 wide.
    let e = heal_at(&cube_with_a_corner_hole(10 * MM, 7000), 6400).unwrap_err();
    let HealError::ToleranceExceeded { boundaries } = &e else {
        panic!("{e:?}");
    };
    assert_eq!(boundaries.len(), 1);
    assert_eq!(boundaries[0].width, Steps(9900));
    assert_eq!(
        e.remedies(GridStep::Q0),
        [Remedy::UseRoute {
            route: "reconstruct"
        }]
    );
    assert!(e.to_string().contains("cap of 6400 steps"));
}

#[test]
fn a_hole_as_wide_as_the_cap_is_still_within_it() {
    // 4,525 √2 = 6,399.4, rounded up to the cap itself: not over it.
    let e = heal_at(&cube_with_a_corner_hole(10 * MM, 4525), 640).unwrap_err();
    let HealError::NotClosed { boundaries } = &e else {
        panic!("{e:?}");
    };
    assert_eq!(boundaries[0].width, Steps(6400));
}

#[test]
fn an_epsilon_over_the_cap_is_refused() {
    let e = heal_at(&cube(MM), 6401).unwrap_err();
    assert_eq!(e, HealError::AboveCap { asked: Steps(6401) });
    assert_eq!(
        e.remedies(GridStep::Q0),
        [Remedy::ChangeInput {
            input: "epsilon",
            to: Some((
                Bound::AtMost,
                Value::Length {
                    steps: Steps(6400),
                    step: GridStep::Q0
                }
            ))
        }]
    );
    assert!(heal_at(&cube(MM), 6400).is_ok());
}

#[test]
fn two_t_junctions_on_one_edge_go_in_in_order() {
    // The front is three faces, meeting at x = side and x = 2 side; the bottom and top each run
    // past both corners, one each way round.
    let s = MM;
    let mut points = corners([0; 3], [3 * s; 3]);
    points.extend([[s, 0, 0], [2 * s, 0, 0], [s, 0, 3 * s], [2 * s, 0, 3 * s]]);
    let mut faces = sides(0);
    faces[2] = vec![0, 8, 10, 4];
    faces.push(vec![8, 9, 11, 10]);
    faces.push(vec![9, 1, 5, 11]);
    let h = heal_at(&raw(&points, &faces), 640).unwrap();
    assert_eq!(h.log.holes.stitched.len(), 4);
    assert_eq!(six_volume(&h.mesh), cube_volume(3 * s));
    assert_eq!(h.mesh.faces(), 6);
}

#[test]
fn two_boxes_on_one_face_are_open_patches_left_as_they_face() {
    // Each box lists the face they share, the other way round from its neighbour's copy: its five
    // other faces, and each copy, are patches open on their own, which no volume turns.
    let mut points = corners([0; 3], [MM; 3]);
    points.extend(corners([MM, 0, 0], [2 * MM, MM, MM]));
    let mut faces = sides(0);
    faces.extend(sides(8));
    let h = heal_at(&raw(&points, &faces), 640).unwrap();
    assert_eq!(h.log.orient.patches, 4);
    assert!(h.log.orient.turned.is_empty());
    assert_eq!(
        six_volume(&h.mesh),
        cube_volume(MM).checked_add(&cube_volume(MM)).unwrap()
    );
}

#[test]
fn a_missing_face_is_one_open_boundary_of_four_corners() {
    let mut faces = sides(0);
    faces.pop();
    let e = heal_at(&raw(&corners([0; 3], [MM; 3]), &faces), 640).unwrap_err();
    // 6,400 √2 = 9,050.97, over the cap.
    assert_eq!(
        e,
        HealError::ToleranceExceeded {
            boundaries: vec![Boundary {
                at: p(0, 0, MM),
                corners: 4,
                width: Steps(9051),
            }]
        }
    );
}

#[test]
fn a_t_junction_is_stitched_and_nothing_moves() {
    // The front is two faces meeting at x = 1; the bottom and top each run past that corner.
    let s = MM;
    let mut points = corners([0; 3], [2 * s; 3]);
    points.extend([[s, 0, 0], [s, 0, 2 * s]]);
    let mut faces = sides(0);
    faces[2] = vec![0, 8, 9, 4];
    faces.push(vec![8, 1, 5, 9]);
    let h = heal_at(&raw(&points, &faces), 640).unwrap();
    assert_eq!(h.mesh.parts().class, Exactness::Exact);
    assert_eq!(h.log.holes.stitched.len(), 2);
    assert_eq!(six_volume(&h.mesh), cube_volume(2 * s));
}

#[test]
fn overlapping_cubes_in_one_mesh_resolve_to_their_union() {
    let mut points = corners([0; 3], [2 * MM; 3]);
    points.extend(corners([MM; 3], [3 * MM; 3]));
    let mut faces = sides(0);
    faces.extend(sides(8));
    let h = heal_at(&raw(&points, &faces), 640).unwrap();
    closed(&h);
    let fifteen = I256::from_i128(6 * 15 * i128::from(MM).pow(3));
    assert_eq!(six_volume(&h.mesh), fifteen);
}

#[test]
fn a_cube_listed_twice_is_one_cube() {
    let mut faces = sides(0);
    faces.extend(sides(0));
    let h = heal_at(&raw(&corners([0; 3], [MM; 3]), &faces), 640).unwrap();
    let repeats: Vec<Removed> = (0..6)
        .map(|f| Removed {
            face: 6 + f,
            why: Removal::Repeated { of: f },
        })
        .collect();
    assert_eq!(h.log.weld.removed, repeats);
    assert_eq!(six_volume(&h.mesh), cube_volume(MM));
}

#[test]
fn a_sheet_listed_both_ways_round_bounds_nothing() {
    let points = [[0, 0, 0], [MM, 0, 0], [MM, MM, 0], [0, MM, 0]];
    let faces = vec![vec![0, 1, 2, 3], vec![3, 2, 1, 0]];
    let h = heal_at(&raw(&points, &faces), 640).unwrap();
    assert_eq!(h.log.orient.flat, [p(0, 0, 0)]);
    assert_eq!(h.mesh.faces(), 0);
}

#[test]
fn a_face_off_its_plane_is_split_into_a_fan_from_its_lowest_corner() {
    // Corner 7 lifted 50 steps: the top is no longer flat (the sides through it still are, on
    // x = side and y = side).
    let mut points = corners([0; 3], [MM; 3]);
    points[7][2] += 50;
    let h = heal_at(&raw(&points, &sides(0)), 0).unwrap();
    let split: Vec<(u32, u32)> = h.log.weld.split.iter().map(|s| (s.face, s.into)).collect();
    assert_eq!(split, [(5, 2)]);
    assert_eq!(h.mesh.parts().class, Exactness::Exact);
    // The volume of the surface fanned from each face's lowest-keyed corner.
    let mut six = I256::from_i128(0);
    for face in sides(0) {
        let at = face
            .iter()
            .position(|&v| {
                let [x, y, z] = points[v as usize];
                face.iter().all(|&w| {
                    let [a, b, c] = points[w as usize];
                    morton(p(x, y, z)) <= morton(p(a, b, c))
                })
            })
            .unwrap();
        let mut looped = face.clone();
        looped.rotate_left(at);
        let q = |v: u32| points[v as usize].map(i128::from);
        let a = q(looped[0]);
        for w in looped[1..].windows(2) {
            let (b, c) = (q(w[0]), q(w[1]));
            let det = a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]);
            six = six.checked_add(&I256::from_i128(det)).unwrap();
        }
    }
    assert_eq!(six_volume(&h.mesh), six);
}

#[test]
fn a_face_with_no_area_is_removed() {
    let mut points = corners([0; 3], [2 * MM; 3]);
    points.push([MM, 0, 0]);
    let mut faces = sides(0);
    faces.push(vec![0, 8, 1]);
    let h = heal_at(&raw(&points, &faces), 640).unwrap();
    assert_eq!(
        h.log.weld.removed,
        [Removed {
            face: 6,
            why: Removal::Degenerate
        }]
    );
    assert_eq!(six_volume(&h.mesh), cube_volume(2 * MM));
}

#[test]
fn a_face_that_turns_back_on_itself_is_split_into_a_fan() {
    // The top runs down its left side to the middle, back up a quarter, and down again: its sides
    // overlap, so it is fanned, and the fan's triangles along that side have no area.
    let mut points = corners([0; 3], [MM; 3]);
    points.extend([[0, MM / 2, MM], [0, 3 * MM / 4, MM]]);
    let mut faces = sides(0);
    faces[5] = vec![4, 5, 7, 6, 8, 9];
    let h = heal_at(&raw(&points, &faces), 640).unwrap();
    let split: Vec<(u32, u32)> = h.log.weld.split.iter().map(|s| (s.face, s.into)).collect();
    assert_eq!(split, [(5, 2)]);
    assert_eq!(h.mesh.parts().class, Exactness::Exact);
    assert_eq!(six_volume(&h.mesh), cube_volume(MM));
}

#[test]
fn a_face_that_crosses_itself_is_split_and_a_simple_one_kept() {
    let weld_one = |points: &[[i64; 3]]| {
        let face = vec![(0..points.len() as u32).collect()];
        weld(&raw(points, &face), Steps(0), &Exec::serial(), &mut meter()).unwrap()
    };
    // A bow tie: its first and third sides cross at the middle.
    let w = weld_one(&[[0, 0, 0], [MM, MM, 0], [MM, 0, 0], [0, MM, 0]]);
    let split: Vec<(u32, u32)> = w.log.split.iter().map(|s| (s.face, s.into)).collect();
    assert_eq!(split, [(0, 2)]);
    // A face whose corner touches a side it does not end, met from either end: the touching
    // corner ends a side after the one it touches, ends one before it, or is the first corner.
    for touching in [
        vec![
            [0, 0, 0],
            [2 * MM, 0, 0],
            [2 * MM, MM, 0],
            [MM, 0, 0],
            [0, MM, 0],
        ],
        vec![
            [3 * MM, 2 * MM, 0],
            [2 * MM, 0, 0],
            [MM, 2 * MM, 0],
            [0, 0, 0],
            [4 * MM, 0, 0],
            [4 * MM, 2 * MM, 0],
        ],
        vec![
            [MM, 0, 0],
            [MM, MM, 0],
            [2 * MM, MM, 0],
            [2 * MM, 0, 0],
            [0, 0, 0],
        ],
    ] {
        let w = weld_one(&touching);
        assert_eq!(w.log.split.len(), 1, "{touching:?}");
    }
    // A face that turns straight back once, its third corner inside its first side, and its
    // mirror image, each way round: one pair of sides meets in each, through a different end of
    // a side each time (the weld starts every loop at its lowest-keyed corner).
    let fold = [
        [0, 0, 0],
        [4 * MM, 0, 0],
        [2 * MM, 0, 0],
        [2 * MM, 3 * MM, 0],
    ];
    let mirror = fold.map(|[x, y, z]| [4 * MM - x, y, z]);
    for shape in [fold, mirror] {
        for reverse in [false, true] {
            let mut loop_points = shape.to_vec();
            if reverse {
                loop_points.reverse();
            }
            let w = weld_one(&loop_points);
            assert_eq!(w.log.split.len(), 1, "{loop_points:?}");
        }
    }
    // An L, not convex but simple, with a corner on a straight side: kept as it is.
    let w = weld_one(&[
        [0, 0, 0],
        [MM, 0, 0],
        [2 * MM, 0, 0],
        [2 * MM, MM, 0],
        [MM, MM, 0],
        [MM, 2 * MM, 0],
        [0, 2 * MM, 0],
    ]);
    assert!(w.log.split.is_empty() && w.log.removed.is_empty());
    assert_eq!(w.mesh.corners().len(), 7);
}

#[test]
fn a_non_orientable_surface_cannot_be_paired() {
    // The tetrahemihexahedron: four of an octahedron's faces and its three middle squares, each
    // edge in one triangle and one square, a projective plane.
    let s = MM;
    let points = [
        [s, 0, 0],
        [-s, 0, 0],
        [0, s, 0],
        [0, -s, 0],
        [0, 0, s],
        [0, 0, -s],
    ];
    let faces = vec![
        vec![0, 2, 4],
        vec![0, 3, 5],
        vec![1, 2, 5],
        vec![1, 3, 4],
        vec![0, 2, 1, 3],
        vec![2, 4, 3, 5],
        vec![4, 0, 5, 1],
    ];
    let e = heal_at(&raw(&points, &faces), 640).unwrap_err();
    let HealError::Unpaired {
        edges,
        non_orientable,
    } = &e
    else {
        panic!("{e:?}");
    };
    assert!(!edges.is_empty());
    assert_eq!(non_orientable.len(), 1);
    assert_eq!(
        e.remedies(GridStep::Q0),
        [Remedy::UseRoute {
            route: "reconstruct"
        }]
    );
}

#[test]
fn the_order_of_vertices_faces_and_corners_does_not_show() {
    let nudge = |f: u32| [5 * i64::from(f), 3 * i64::from(f), 0];
    let first = soup_cube(MM, nudge);
    // The same faces backwards, each loop started elsewhere, the vertices listed backwards.
    let n = first.vertices().len() as u32;
    let points: Vec<[i64; 3]> = first
        .vertices()
        .iter()
        .rev()
        .map(|v| [v.x.get(), v.y.get(), v.z.get()])
        .collect();
    let faces: Vec<Vec<u32>> = first
        .face_start()
        .windows(2)
        .rev()
        .map(|w| {
            let mut f: Vec<u32> = first.corners()[w[0] as usize..w[1] as usize]
                .iter()
                .map(|&c| n - 1 - c)
                .collect();
            f.rotate_left(1);
            f
        })
        .collect();
    let second = raw(&points, &faces);
    let (a, b) = (
        heal_at(&first, 640).unwrap(),
        heal_at(&second, 640).unwrap(),
    );
    assert_eq!(a.mesh, b.mesh);
    assert_eq!(a.log.weld.movement, b.log.weld.movement);
    assert_eq!(a.log.weld.joined, b.log.weld.joined);
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn the_threads_do_not_show() {
    let pool = Rayon::new(NonZeroUsize::new(4).unwrap()).unwrap();
    let mesh = cube_with_a_corner_hole(MM, 300);
    let settings = Heal::default();
    let go =
        |exec: &Exec<'_>| heal(&mesh, settings, source(), &step(), exec, &mut meter()).unwrap();
    assert_eq!(go(&Exec::serial()), go(&Exec::with_pool(&pool)));
}

#[test]
fn the_default_is_640_steps_of_q0() {
    assert_eq!(
        Heal::default(),
        Heal {
            epsilon: Steps(640),
            step: GridStep::Q0
        }
    );
}

#[test]
fn nothing_heals_to_nothing() {
    let h = heal_at(&raw(&[], &[]), 640).unwrap();
    assert_eq!(h.mesh.faces(), 0);
    assert_eq!(h.mesh.parts().class, Exactness::Exact);
}

#[test]
fn a_small_budget_stops_healing() {
    let mut short = WorkMeter::new(Budget::of_work(10));
    let e = heal(
        &soup_cube(MM, |f| [i64::from(f), 0, 0]),
        Heal::default(),
        source(),
        &step(),
        &Exec::serial(),
        &mut short,
    )
    .unwrap_err();
    assert!(
        matches!(
            e,
            HealError::Mesh(MeshError::Stopped(_) | MeshError::BudgetExceeded(_))
        ),
        "{e:?}"
    );
}

#[test]
fn quantise_rounds_each_exact_product_once_ties_to_even() {
    let q = quantise(
        &[[0.5, 1.5, 2.5], [-0.5, -1.5, 1.25], [1e-300, 0.0, 3.0]],
        1,
        vec![0, 1, 2],
        vec![0, 3],
    )
    .unwrap();
    let got: Vec<[i64; 3]> = q
        .mesh
        .vertices()
        .iter()
        .map(|v| [v.x.get(), v.y.get(), v.z.get()])
        .collect();
    assert_eq!(got, [[0, 2, 2], [0, -2, 1], [0, 0, 3]]);
    assert_eq!(q.moved, 7);
    assert_eq!(q.largest, 0.5);
}

#[test]
fn quantise_scales_by_a_whole_unit_exactly() {
    // 0.1 mm is not a double, but 0.1 × 6,400 rounds to 640 steps, and 1/3 mm to 2,133.
    let q = quantise(&[[0.1, 1.0 / 3.0, 25.4]], 6400, vec![], vec![0]).unwrap();
    let v = q.mesh.vertices()[0];
    assert_eq!([v.x.get(), v.y.get(), v.z.get()], [640, 2133, 162_560]);
    assert_eq!(q.moved, 3);
    assert!(q.largest > 0.3 && q.largest < 0.34);
}

#[test]
fn quantise_refuses_what_is_no_point_of_the_frame() {
    let corners = || (vec![], vec![0]);
    let (c, s) = corners();
    assert_eq!(
        quantise(&[[0.0, f64::NAN, 0.0]], 1, c, s).unwrap_err(),
        HealError::NotFinite { vertex: 0 }
    );
    let (c, s) = corners();
    let edge = (1_i64 << 40) as f64;
    assert_eq!(
        quantise(&[[0.0; 3], [edge, 0.0, 0.0]], 1, c, s).unwrap_err(),
        HealError::OutOfFrame { vertex: 1 }
    );
    let (c, s) = corners();
    assert_eq!(
        quantise(&[[1e300, 0.0, 0.0]], 4_000_000_000, c, s).unwrap_err(),
        HealError::OutOfFrame { vertex: 0 }
    );
    let (c, s) = corners();
    assert_eq!(
        quantise(&[[0.0; 3]], 0, c, s).unwrap_err(),
        HealError::NoUnit
    );
    let (c, s) = corners();
    assert!(quantise(&[[edge - 1.0, 0.0, 0.0]], 1, c, s).is_ok());
    assert!(matches!(
        quantise(&[[0.0; 3]], 1, vec![0, 1, 2], vec![0, 3]),
        Err(HealError::Mesh(MeshError::Fault(_)))
    ));
}

#[test]
fn errors_say_what_was_wrong() {
    for (e, words) in [
        (HealError::NotFinite { vertex: 3 }, "vertex 3"),
        (HealError::OutOfFrame { vertex: 4 }, "outside the frame"),
        (HealError::NoUnit, "zero steps"),
        (HealError::AboveCap { asked: Steps(7000) }, "7000 steps"),
        (
            HealError::Unpaired {
                edges: vec![],
                non_orientable: vec![],
            },
            "cannot be paired",
        ),
        (HealError::Mesh(MeshError::Defect), "defect"),
    ] {
        assert!(e.to_string().contains(words), "{e}");
    }
    assert!(HealError::NoUnit.remedies(GridStep::Q0).is_empty());
    assert!(
        HealError::Mesh(MeshError::Defect)
            .remedies(GridStep::Q0)
            .is_empty()
    );
}
