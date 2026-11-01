//! Attacks on chapter 8.3's groups 2 and 3 for plane-based bodies (G134): vertices where three
//! planes meet, faces of any shape, on planes that cross, on one plane, and shells that nest.
//!
//! The polyhedra here have three faces at every vertex, so each can be tilted: every face's plane
//! moved a step or so off its grid points, and every vertex the meeting of its three planes, at a
//! place no grid point holds. Moving a plane so little changes no shape, so every tilted polyhedron
//! must pass, and each fault planted in one must be found, by name.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::redundant_closure_for_method_calls,
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "test fixtures; the rules are for kernel code"
)]

mod common;

use common::{
    CUBE_FACES, Rows, certified, faces_into, ix, kinds, level, line, meter, polyhedron, report,
};
use geom_check::{Entity, Fault, FaultKind, Level, Report, check};
use geom_curve::CurveKind;
use geom_math::Box3;
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_run::{Budget, Stop, WorkMeter};
use geom_surface::{Plane, SurfaceKind};
use geom_topo::{Body, BodyKind, RegionKind, Sense};

/// A polyhedron: its corners, and for each solid region its shells, each a list of faces, each a
/// list of loops of corner numbers, counter-clockwise seen from outside and holes clockwise.
#[derive(Clone, Copy)]
struct Spec<'a> {
    points: &'a [[i64; 3]],
    regions: &'a [&'a [&'a [&'a [&'a [u32]]]]],
}

/// Side 100, corners by bits: bit 0 x, bit 1 y, bit 2 z.
const fn corners(o: [i64; 3], s: [i64; 3]) -> [[i64; 3]; 8] {
    let [x, y, z] = o;
    let [a, b, c] = s;
    [
        [x, y, z],
        [x + a, y, z],
        [x, y + b, z],
        [x + a, y + b, z],
        [x, y, z + c],
        [x + a, y, z + c],
        [x, y + b, z + c],
        [x + a, y + b, z + c],
    ]
}

const BOX_POINTS: [[i64; 3]; 8] = corners([0, 0, 0], [100, 100, 100]);

/// The faces of a box from corner `n`, counter-clockwise from outside.
macro_rules! box_faces {
    ($n:expr) => {
        [
            &[&[$n, $n + 2, $n + 3, $n + 1]],
            &[&[$n + 4, $n + 5, $n + 7, $n + 6]],
            &[&[$n, $n + 1, $n + 5, $n + 4]],
            &[&[$n + 2, $n + 6, $n + 7, $n + 3]],
            &[&[$n, $n + 4, $n + 6, $n + 2]],
            &[&[$n + 1, $n + 3, $n + 7, $n + 5]],
        ]
    };
}

/// The same box inside out, a cavity's shell.
macro_rules! cavity_faces {
    ($n:expr) => {
        [
            &[&[$n, $n + 1, $n + 3, $n + 2]],
            &[&[$n + 4, $n + 6, $n + 7, $n + 5]],
            &[&[$n, $n + 4, $n + 5, $n + 1]],
            &[&[$n + 2, $n + 3, $n + 7, $n + 6]],
            &[&[$n, $n + 2, $n + 6, $n + 4]],
            &[&[$n + 1, $n + 5, $n + 7, $n + 3]],
        ]
    };
}

const BOX_SHELL: [&[&[u32]]; 6] = box_faces!(0);

const BOX: Spec<'_> = Spec {
    points: &BOX_POINTS,
    regions: &[&[&BOX_SHELL]],
};

/// An L, extruded along z from 0 to 100: not convex, on either cap.
const ELL_POINTS: [[i64; 3]; 12] = [
    [0, 0, 0],
    [200, 0, 0],
    [200, 100, 0],
    [100, 100, 0],
    [100, 200, 0],
    [0, 200, 0],
    [0, 0, 100],
    [200, 0, 100],
    [200, 100, 100],
    [100, 100, 100],
    [100, 200, 100],
    [0, 200, 100],
];

const ELL_SHELL: [&[&[u32]]; 8] = [
    &[&[0, 5, 4, 3, 2, 1]],
    &[&[6, 7, 8, 9, 10, 11]],
    &[&[0, 1, 7, 6]],
    &[&[1, 2, 8, 7]],
    &[&[2, 3, 9, 8]],
    &[&[3, 4, 10, 9]],
    &[&[4, 5, 11, 10]],
    &[&[5, 0, 6, 11]],
];

const ELL: Spec<'_> = Spec {
    points: &ELL_POINTS,
    regions: &[&[&ELL_SHELL]],
};

/// A wall with a window through it: two faces with holes.
const WALL_POINTS: [[i64; 3]; 16] = [
    [0, 0, 0],
    [400, 0, 0],
    [0, 0, 300],
    [400, 0, 300],
    [0, 50, 0],
    [400, 50, 0],
    [0, 50, 300],
    [400, 50, 300],
    [100, 0, 100],
    [300, 0, 100],
    [100, 0, 200],
    [300, 0, 200],
    [100, 50, 100],
    [300, 50, 100],
    [100, 50, 200],
    [300, 50, 200],
];

const WALL_SHELL: [&[&[u32]]; 10] = [
    &[&[0, 1, 3, 2], &[8, 10, 11, 9]],
    &[&[4, 6, 7, 5], &[12, 13, 15, 14]],
    &[&[0, 4, 5, 1]],
    &[&[2, 3, 7, 6]],
    &[&[0, 2, 6, 4]],
    &[&[1, 5, 7, 3]],
    &[&[8, 9, 13, 12]],
    &[&[10, 14, 15, 11]],
    &[&[8, 12, 14, 10]],
    &[&[9, 11, 15, 13]],
];

const WALL: Spec<'_> = Spec {
    points: &WALL_POINTS,
    regions: &[&[&WALL_SHELL]],
};

/// A box and a second, apart.
const TWO_POINTS: [[i64; 3]; 16] = {
    let (a, b) = (
        corners([0, 0, 0], [100, 100, 100]),
        corners([300, 0, 0], [100, 100, 100]),
    );
    [
        a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], b[0], b[1], b[2], b[3], b[4], b[5], b[6],
        b[7],
    ]
};

const SECOND_SHELL: [&[&[u32]]; 6] = box_faces!(8);

const TWO: Spec<'_> = Spec {
    points: &TWO_POINTS,
    regions: &[&[&BOX_SHELL], &[&SECOND_SHELL]],
};

/// A box with a box-shaped cavity, and a third box floating in the cavity.
const NEST_POINTS: [[i64; 3]; 24] = {
    let (a, b, c) = (
        corners([0, 0, 0], [900, 900, 900]),
        corners([100, 100, 100], [700, 700, 700]),
        corners([300, 300, 300], [300, 300, 300]),
    );
    [
        a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], b[0], b[1], b[2], b[3], b[4], b[5], b[6],
        b[7], c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7],
    ]
};

const CAVITY_SHELL: [&[&[u32]]; 6] = cavity_faces!(8);
const FLOATING_SHELL: [&[&[u32]]; 6] = box_faces!(16);

const NEST: Spec<'_> = Spec {
    points: &NEST_POINTS,
    regions: &[&[&BOX_SHELL, &CAVITY_SHELL], &[&FLOATING_SHELL]],
};

const CORPUS: [Spec<'_>; 5] = [BOX, ELL, WALL, TWO, NEST];

/// Three corners of `ring` that turn the way it runs, by the ring's area vector.
fn anchors(points: &[[i64; 3]], ring: &[u32]) -> [[i64; 3]; 3] {
    let at = |v: u32| points[v as usize].map(i128::from);
    let cross = |p: [i128; 3], q: [i128; 3]| {
        [
            p[1] * q[2] - p[2] * q[1],
            p[2] * q[0] - p[0] * q[2],
            p[0] * q[1] - p[1] * q[0],
        ]
    };
    let minus = |p: [i128; 3], q: [i128; 3]| [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
    let n = ring.len();
    let mut area = [0_i128; 3];
    for k in 0..n {
        let c = cross(at(ring[k]), at(ring[(k + 1) % n]));
        area = [0, 1, 2].map(|i| area[i] + c[i]);
    }
    for k in 0..n {
        let (a, b, c) = (ring[k], ring[(k + 1) % n], ring[(k + 2) % n]);
        let turn = cross(minus(at(b), at(a)), minus(at(c), at(a)));
        if turn[0] * area[0] + turn[1] * area[1] + turn[2] * area[2] > 0 {
            return [a, b, c].map(|v| points[v as usize]);
        }
    }
    panic!("a ring with no convex corner")
}

/// `spec`'s body, every vertex the meeting of its three faces' planes, each plane through its
/// face's anchors moved by `jog(face, anchor)`.
fn tilted(spec: Spec<'_>, jog: &dyn Fn(usize, usize) -> [i64; 3]) -> Rows {
    let mut r = Rows::empty();
    let faces: Vec<&[&[u32]]> = spec
        .regions
        .iter()
        .flat_map(|shells| shells.iter())
        .flat_map(|faces| faces.iter().copied())
        .collect();
    let mut on: Vec<[[i64; 3]; 3]> = Vec::new();
    for (k, loops) in faces.iter().enumerate() {
        let moved = anchors(spec.points, loops[0]);
        let mut a = 0;
        let moved = moved.map(|p| {
            let by = jog(k, a);
            a += 1;
            [p[0] + by[0], p[1] + by[1], p[2] + by[2]]
        });
        r.plane_through(moved);
        on.push(moved);
    }
    for v in 0..spec.points.len() as u32 {
        let mut planes = faces
            .iter()
            .enumerate()
            .filter(|(_, loops)| loops.iter().any(|ring| ring.contains(&v)))
            .map(|(k, _)| k as u32);
        let triple = [
            planes.next().unwrap(),
            planes.next().unwrap(),
            planes.next().unwrap(),
        ];
        assert_eq!(planes.next(), None, "three faces at a vertex");
        r.meet(triple);
    }
    let mut face = 0;
    for shells in spec.regions {
        let solid = r.region(RegionKind::Solid);
        for shell in *shells {
            let s = r.shell(solid);
            for loops in *shell {
                let rings: Vec<Vec<(u32, Sense)>> =
                    loops.iter().map(|ring| fins(&mut r, ring)).collect();
                let rings: Vec<&[(u32, Sense)]> = rings.iter().map(Vec::as_slice).collect();
                r.face_on(s, &rings, on[face]);
                face += 1;
            }
        }
    }
    r
}

/// The fins of `ring`, each edge made the first time its two vertices meet.
fn fins(r: &mut Rows, ring: &[u32]) -> Vec<(u32, Sense)> {
    let n = ring.len();
    (0..n)
        .map(|k| {
            let (a, b) = (ring[k], ring[(k + 1) % n]);
            let key = (a.min(b), a.max(b));
            let found = (0..r.edges.len()).find(|&e| {
                r.edges[e].start.map(|v| v.row()) == Some(key.0)
                    && r.edges[e].end.map(|v| v.row()) == Some(key.1)
            });
            let e = found.map_or_else(|| r.edge(Some(key)), |e| e as u32);
            (
                e,
                if a < b {
                    Sense::Forward
                } else {
                    Sense::Reversed
                },
            )
        })
        .collect()
}

/// No move at all: the planes through the grid points themselves.
fn still(_: usize, _: usize) -> [i64; 3] {
    [0; 3]
}

/// A step or so each way, different at every anchor of every face, from `seed`.
fn jog(seed: u64) -> impl Fn(usize, usize) -> [i64; 3] {
    move |face, anchor| {
        let mix = |k: u64| {
            let x = seed
                .wrapping_add((face as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15))
                .wrapping_add((anchor as u64 + 1).wrapping_mul(0xBF58_476D_1CE4_E5B9))
                .wrapping_add(k.wrapping_mul(0x94D0_49BB_1331_11EB));
            let x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            ((x ^ (x >> 27)) % 3) as i64 - 1
        };
        [mix(0), mix(1), mix(2)]
    }
}

/// The seed of the random corpus: fixed, printed, and replaced by `SPAREKIT_SEED` in hex.
fn seed() -> u64 {
    let seed = std::env::var("SPAREKIT_SEED").map_or(0x6134_6134, |s| {
        u64::from_str_radix(&s, 16).expect("SPAREKIT_SEED in hex")
    });
    println!("seed: {seed:x}");
    seed
}

/// The faults of `body`, at the full level.
fn full(body: &Body, exactness: Exactness) -> Report {
    check(body, exactness, Level::Full, &mut meter()).unwrap()
}

/// The faults of `rows`, a solid recorded Exact.
fn faults(rows: &Rows) -> Vec<Fault> {
    full(&rows.body(BodyKind::Solid), Exactness::Exact)
        .faults()
        .to_vec()
}

#[test]
fn the_planes_through_the_grid_points_pass_as_they_are() {
    for spec in CORPUS {
        let found = faults(&tilted(spec, &still));
        assert!(found.is_empty(), "{found:?}");
    }
}

#[test]
fn every_tilted_polyhedron_passes() {
    let seed = seed();
    for k in 0..40 {
        for spec in CORPUS {
            let rows = tilted(spec, &jog(seed.wrapping_add(k)));
            let found = faults(&rows);
            assert!(found.is_empty(), "seed {seed:x} + {k}: {found:?}");
        }
    }
}

#[test]
fn a_tilted_body_has_vertices_no_grid_point_holds() {
    let rows = tilted(BOX, &jog(7));
    let body = rows.body(BodyKind::Solid);
    let implicit = (0..8).filter(|&v| {
        matches!(
            geom_topo::query::point(&body, ix(v)),
            Err(geom_topo::query::QueryError::Implicit(_))
        )
    });
    assert_eq!(implicit.count(), 8);
}

#[test]
fn a_cut_corner_mixes_grid_vertices_and_meetings() {
    // A cube of side 30 with its far corner cut by 3x + 4y + 7z = 370, which meets the three edges
    // there at z = 160/7, y = 35/2 and x = 40/3.
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let corner = [
        [0, 0, 0],
        [30, 0, 0],
        [0, 30, 0],
        [30, 30, 0],
        [0, 0, 30],
        [30, 0, 30],
        [0, 30, 30],
    ];
    for p in corner {
        r.vertex(p);
    }
    let high_x = [[30, 0, 0], [30, 30, 0], [30, 0, 30]];
    let high_y = [[0, 30, 0], [0, 30, 30], [30, 30, 0]];
    let high_z = [[0, 0, 30], [30, 0, 30], [0, 30, 30]];
    let cut = [[20, 25, 30], [24, 22, 30], [26, 24, 28]];
    let x = r.plane_through(high_x);
    let y = r.plane_through(high_y);
    let z = r.plane_through(high_z);
    let c = r.plane_through(cut);
    let a = r.meet([x, y, c]);
    let b = r.meet([x, z, c]);
    let d = r.meet([y, z, c]);
    let s = r.shell(solid);
    let low_z = [[0, 0, 0], [0, 30, 0], [30, 0, 0]];
    let low_y = [[0, 0, 0], [30, 0, 0], [0, 0, 30]];
    let low_x = [[0, 0, 0], [0, 0, 30], [0, 30, 0]];
    let faces: [(&[u32], [[i64; 3]; 3]); 7] = [
        (&[0, 2, 3, 1], low_z),
        (&[0, 1, 5, 4], low_y),
        (&[0, 4, 6, 2], low_x),
        (&[1, 3, a, b, 5], high_x),
        (&[2, 6, d, a, 3], high_y),
        (&[4, 5, b, d, 6], high_z),
        (&[a, d, b], cut),
    ];
    for (ring, on) in faces {
        let ring = fins(&mut r, ring);
        r.face_on(s, &[&ring], on);
    }
    assert_eq!(faults(&r), []);
}

#[test]
fn three_planes_that_do_not_meet_are_a_fault() {
    let mut r = tilted(BOX, &still);
    // Faces 0 and 1 are the box's two parallel faces.
    r.meets[0].1 = [0, 1, 2];
    let found = faults(&r);
    assert_eq!(
        found.first(),
        Some(&Fault::at(FaultKind::MeetSingular, Entity::Vertex(ix(0))))
    );
}

#[test]
fn a_meeting_off_a_face_plane_is_a_fault_in_every_class() {
    let mut r = tilted(BOX, &jog(3));
    // Vertex 0 is given vertex 7's planes, which are the far three.
    r.meets[0].1 = r.meets[7].1;
    let body = r.body(BodyKind::Solid);
    let certified = Exactness::Certified {
        bound: Steps(1_000),
        step: GridStep::Q0,
    };
    for exactness in [Exactness::Exact, certified] {
        let report = full(&body, exactness);
        assert!(
            kinds(report.faults()).contains(&FaultKind::VertexOffSurface),
            "{:?}",
            report.faults()
        );
    }
}

#[test]
fn a_shadow_line_a_long_way_from_its_vertex_is_a_fault() {
    let mut r = tilted(BOX, &jog(3));
    let p = r.points[0];
    let q = r.points[1];
    let at = |p: P3| [p.x.get(), p.y.get(), p.z.get()];
    let e = (0..r.edges.len())
        .find(|&e| r.edges[e].start.map(|v| v.row()) == Some(0))
        .unwrap();
    let curve = r.edges[e].curve.row() as usize;
    let mut far = at(p);
    far[2] += 5;
    r.curves[curve] = line(far, at(q));
    assert!(kinds(&faults(&r)).contains(&FaultKind::VertexOffCurve));
}

#[test]
fn a_shadow_line_a_step_off_passes() {
    let mut r = tilted(BOX, &still);
    let at = |p: P3| [p.x.get(), p.y.get(), p.z.get()];
    let e = (0..r.edges.len())
        .find(|&e| r.edges[e].start.map(|v| v.row()) == Some(0))
        .unwrap();
    let curve = r.edges[e].curve.row() as usize;
    let end = r.edges[e].end.unwrap().row() as usize;
    let (start, end) = (at(r.points[0]), at(r.points[end]));
    r.curves[curve] = line([start[0] + 1, start[1], start[2]], [end[0], end[1], end[2]]);
    assert_eq!(faults(&r), []);
}

/// Adds a plane of constant x and one of constant y; the vertex where they meet z = 0, plane `z`.
fn flat_corner(r: &mut Rows, z: u32, x: i64, y: i64) -> u32 {
    let across = r.plane_through([[x, 0, 0], [x, 1, 0], [x, 0, 1]]);
    let along = r.plane_through([[0, y, 0], [0, y, 1], [1, y, 0]]);
    r.meet([z, across, along])
}

const FLOOR: [[i64; 3]; 3] = [[0, 0, 0], [100, 0, 0], [0, 100, 0]];

/// A sheet on z = 0 with rings of corners in the plan given, every corner a meeting of planes.
fn plan(rings: &[&[[i64; 2]]]) -> Rows {
    let mut r = Rows::empty();
    let z = r.plane_through(FLOOR);
    let s = r.shell(0);
    let mut numbered: Vec<([i64; 2], u32)> = Vec::new();
    let rings: Vec<Vec<u32>> = rings
        .iter()
        .map(|ring| {
            ring.iter()
                .map(|&q| {
                    if let Some(&(_, v)) = numbered.iter().find(|(p, _)| *p == q) {
                        return v;
                    }
                    let v = flat_corner(&mut r, z, q[0], q[1]);
                    numbered.push((q, v));
                    v
                })
                .collect()
        })
        .collect();
    let rings: Vec<Vec<(u32, Sense)>> = rings.iter().map(|ring| fins(&mut r, ring)).collect();
    let rings: Vec<&[(u32, Sense)]> = rings.iter().map(Vec::as_slice).collect();
    r.face_on(s, &rings, FLOOR);
    r
}

/// The faults of a plan, a sheet recorded Exact.
fn plan_faults(rings: &[&[[i64; 2]]]) -> Vec<Fault> {
    let body = plan(rings).body(BodyKind::Sheet);
    full(&body, Exactness::Exact).faults().to_vec()
}

#[test]
fn a_plan_of_meetings_passes_with_a_hole_and_a_corner_in_a_straight_line() {
    let square: &[[i64; 2]] = &[[0, 0], [50, 0], [100, 0], [100, 100], [0, 100]];
    let hole: &[[i64; 2]] = &[[40, 40], [40, 60], [60, 60], [60, 40]];
    assert_eq!(plan_faults(&[square, hole]), []);
}

#[test]
fn two_edges_of_a_face_crossing_are_found() {
    let found = plan_faults(&[&[[0, 10], [60, 10], [60, 40], [30, 40], [30, 0], [0, 0]]]);
    assert!(kinds(&found).contains(&FaultKind::EdgesCross), "{found:?}");
}

#[test]
fn a_hole_lying_along_the_outer_loop_is_found() {
    let square: &[[i64; 2]] = &[[0, 0], [100, 0], [100, 100], [0, 100]];
    let hole: &[[i64; 2]] = &[[40, 0], [40, 50], [60, 50], [60, 0]];
    let found = plan_faults(&[square, hole]);
    assert!(kinds(&found).contains(&FaultKind::EdgesCross), "{found:?}");
}

#[test]
fn an_edge_folded_back_along_its_neighbour_is_found() {
    let found = plan_faults(&[&[[0, 0], [100, 0], [50, 0], [50, 100], [0, 100]]]);
    assert!(kinds(&found).contains(&FaultKind::EdgesCross), "{found:?}");
}

#[test]
fn an_edge_on_no_plane_of_its_ends_fails_closed() {
    // The edge from (0, 0) to (100, 100) lies on none of the planes its ends meet on.
    let found = plan_faults(&[&[[0, 0], [100, 100], [0, 100]]]);
    assert!(
        kinds(&found).contains(&FaultKind::PairUnchecked),
        "{found:?}"
    );
}

#[test]
fn a_box_is_held_to_a_meetings_exact_place() {
    // A triangle on z = 30 with a corner at (40/3, 30, 30), which rounds to x = 13: a box to x = 14
    // holds it, and a box to x = 13, which holds its rounded place, does not.
    let mut r = Rows::empty();
    let s = r.shell(0);
    let y = r.plane_through([[0, 30, 0], [0, 30, 30], [30, 30, 0]]);
    let z = r.plane_through([[0, 0, 30], [30, 0, 30], [0, 30, 30]]);
    let c = r.plane_through([[20, 25, 30], [24, 22, 30], [26, 24, 28]]);
    let d = r.meet([y, z, c]);
    // (12, 31, 30) is on the cut plane with the meeting, and (12, 30, 30) on y = 30 with it.
    let others = [r.vertex([12, 31, 30]), r.vertex([12, 30, 30])];
    let ring = fins(&mut r, &[d, others[0], others[1]]);
    let f = r.face_on(s, &[&ring], [[0, 0, 30], [30, 0, 30], [0, 30, 30]]);
    let reach = |x: i64| Box3::new(P3::new(11, 24, 29).unwrap(), P3::new(x, 32, 31).unwrap());
    r.faces[f as usize].bound = reach(14).unwrap();
    assert_eq!(
        full(&r.body(BodyKind::Sheet), Exactness::Exact).faults(),
        []
    );
    r.faces[f as usize].bound = reach(13).unwrap();
    let found = full(&r.body(BodyKind::Sheet), Exactness::Exact);
    assert_eq!(kinds(found.faults()), [FaultKind::FaceBound]);
}

#[test]
fn a_lump_through_another_is_found() {
    // Two tilted boxes, the second moved until it runs through the first.
    let points: Vec<[i64; 3]> = TWO_POINTS
        .iter()
        .enumerate()
        .map(|(k, p)| {
            if k < 8 {
                *p
            } else {
                [p[0] - 250, p[1] + 25, p[2] + 25]
            }
        })
        .collect();
    let spec = Spec {
        points: &points,
        regions: TWO.regions,
    };
    let found = faults(&tilted(spec, &jog(11)));
    assert!(kinds(&found).contains(&FaultKind::FacesCross), "{found:?}");
}

#[test]
fn a_tooth_through_a_window_and_a_tooth_into_the_wall_differ() {
    // A box through the wall's window, not touching it, passes as far as faces go.
    let mut points = WALL_POINTS.to_vec();
    points.extend(corners([150, -50, 125], [100, 150, 50]));
    let tooth: [&[&[u32]]; 6] = box_faces!(16);
    let regions: [&[&[&[&[u32]]]]; 2] = [&[&WALL_SHELL], &[&tooth]];
    let spec = Spec {
        points: &points,
        regions: &regions,
    };
    let through = faults(&tilted(spec, &jog(13)));
    assert!(
        !kinds(&through).contains(&FaultKind::FacesCross),
        "{through:?}"
    );
    // Moved up until it runs through the window's head, it crosses.
    let mut points = WALL_POINTS.to_vec();
    points.extend(corners([150, -50, 175], [100, 150, 50]));
    let spec = Spec {
        points: &points,
        regions: &regions,
    };
    let into = faults(&tilted(spec, &jog(13)));
    assert!(kinds(&into).contains(&FaultKind::FacesCross), "{into:?}");
}

#[test]
fn a_shell_turned_inside_out_is_found_by_its_certified_volume() {
    let spec_faces: [&[&[u32]]; 6] = cavity_faces!(0);
    let spec = Spec {
        points: &BOX_POINTS,
        regions: &[&[&spec_faces]],
    };
    let found = faults(&tilted(spec, &jog(17)));
    assert!(kinds(&found).contains(&FaultKind::ShellInward), "{found:?}");
}

#[test]
fn a_cavity_outside_its_shell_is_found_by_the_exact_ray() {
    let points: Vec<[i64; 3]> = NEST_POINTS[..16]
        .iter()
        .enumerate()
        .map(|(k, p)| if k < 8 { *p } else { [p[0] + 2000, p[1], p[2]] })
        .collect();
    let spec = Spec {
        points: &points,
        regions: &[&[&BOX_SHELL, &CAVITY_SHELL]],
    };
    let found = faults(&tilted(spec, &jog(19)));
    assert_eq!(kinds(&found), [FaultKind::ShellNesting], "{found:?}");
}

#[test]
fn a_lump_inside_another_is_found_by_the_exact_ray() {
    let points: Vec<[i64; 3]> = TWO_POINTS
        .iter()
        .enumerate()
        .map(|(k, p)| {
            if k < 8 {
                [p[0] * 9, p[1] * 9, p[2] * 9]
            } else {
                [p[0] - 200, p[1] + 300, p[2] + 300]
            }
        })
        .collect();
    let spec = Spec {
        points: &points,
        regions: TWO.regions,
    };
    let found = faults(&tilted(spec, &jog(23)));
    assert!(
        kinds(&found).contains(&FaultKind::ShellNesting),
        "{found:?}"
    );
}

#[test]
fn a_loop_run_backwards_is_found_by_its_certified_area() {
    let mut r = tilted(BOX, &jog(29));
    r.faces[0].sense = Sense::Reversed;
    assert!(kinds(&faults(&r)).contains(&FaultKind::LoopSense));
}

/// A sheet on z = 0 of grid corners, a square with a square hole.
fn ring_on_the_grid() -> Rows {
    let mut r = Rows::empty();
    let s = r.shell(0);
    let mut ring = |points: &[[i64; 3]]| {
        let numbers: Vec<u32> = points.iter().map(|p| r.vertex(*p)).collect();
        fins(&mut r, &numbers)
    };
    let outer = ring(&[[0, 0, 0], [100, 0, 0], [100, 100, 0], [0, 100, 0]]);
    let hole = ring(&[[40, 40, 0], [40, 60, 0], [60, 60, 0], [60, 40, 0]]);
    r.face_on(s, &[&outer, &hole], FLOOR);
    r
}

/// A sheet on z = 0 of meetings, a square with a square hole.
fn ring_of_meetings() -> Rows {
    plan(&[
        &[[0, 0], [100, 0], [100, 100], [0, 100]],
        &[[40, 40], [40, 60], [60, 60], [60, 40]],
    ])
}

#[test]
fn a_corner_resting_on_a_face_is_found_from_either_face() {
    // No vertex in common, so nothing excuses the touch, whichever face is asked first.
    let square: &[[i64; 3]] = &[[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]];
    let tooth: &[[i64; 3]] = &[[5, 5, 0], [8, 5, 6], [2, 5, 6]];
    for rings in [[tooth, square], [square, tooth]] {
        let found = report(&sheets(&rings), BodyKind::General, Exactness::Exact);
        assert_eq!(kinds(found.faults()), [FaultKind::FacesCross]);
    }
}

#[test]
fn two_faces_along_one_line_with_only_its_ends_in_common_are_found() {
    // Two corners in common and an edge each between them: the shared corners excuse their own
    // places and nothing in between.
    let r = sheets(&[
        &[[0, 0, 0], [4, 0, 0], [10, 0, 0], [5, 5, 0]],
        &[[0, 0, 0], [6, 0, 0], [10, 0, 0], [5, 0, 5]],
    ]);
    let found = report(&r, BodyKind::General, Exactness::Exact);
    assert_eq!(kinds(found.faults()), [FaultKind::FacesCross]);
}

#[test]
fn a_corner_in_the_middle_of_an_edge_of_its_own_face_is_found() {
    // The way down x = 60 turns at (60, 0), which the edge along y = 0 passes through without ending
    // there: nothing they share excuses it, though each of the two has an end there.
    let found = plan_faults(&[&[
        [60, 100],
        [60, 0],
        [60, -100],
        [0, -100],
        [0, 0],
        [100, 0],
        [100, 100],
    ]]);
    assert!(kinds(&found).contains(&FaultKind::EdgesCross), "{found:?}");
}

#[test]
fn a_face_folded_flat_along_one_line_is_found() {
    // Every corner on one line: each pair of edges lies along the other.
    let found = plan_faults(&[&[[0, 0], [100, 0], [50, 0]]]);
    assert!(kinds(&found).contains(&FaultKind::EdgesCross), "{found:?}");
}

#[test]
fn a_bent_face_with_a_second_loop_has_no_fan() {
    // A fan runs round one ring. This second loop turns on from the first as seen from its first
    // corner, so that a fan through both would pass every test of a fan: it must still have none,
    // and the face beside it cannot be told apart from it.
    let mut r = Rows::empty();
    let shell = r.shell(0);
    let numbers = |r: &mut Rows, ring: &[[i64; 3]]| -> Vec<u32> {
        ring.iter().map(|p| r.vertex(*p)).collect()
    };
    let outer = numbers(&mut r, &[[0, 0, 0], [10, 0, 0], [10, 10, 1], [0, 10, 0]]);
    let inner = numbers(&mut r, &[[-1, 10, 0], [-2, 9, 0], [-3, 8, 0], [-4, 6, 0]]);
    let (outer, inner) = (fins(&mut r, &outer), fins(&mut r, &inner));
    r.face(shell, &[&outer, &inner]);
    let shell = r.shell(0);
    let beside = numbers(&mut r, &[[20, 0, -5], [20, 10, -5], [20, 10, 5], [20, 0, 5]]);
    let beside = fins(&mut r, &beside);
    r.face(shell, &[&beside]);
    r.surfaces[0] = off_grid();
    let found = report(&r, BodyKind::General, certified());
    assert_eq!(kinds(found.faults()), [FaultKind::PairUnchecked]);
}

#[test]
fn what_a_check_reserves_is_what_its_lists_hold() {
    // Logical memory is budgeted as work is: a face's lists are reserved by their lengths, here to
    // the byte.
    for (body, bytes) in [
        (ring_on_the_grid().body(BodyKind::Sheet), RING_BYTES),
        (ring_of_meetings().body(BodyKind::Sheet), MEETINGS_BYTES),
    ] {
        let mut m = WorkMeter::new(Budget::UNLIMITED);
        check(&body, Exactness::Exact, Level::Full, &mut m).unwrap();
        assert_eq!(m.used().logical_bytes, bytes);
    }
}

#[test]
fn a_face_turned_with_its_one_hole_is_found() {
    // Its hole now runs counter-clockwise and its outer loop clockwise: one of each.
    for mut r in [ring_on_the_grid(), ring_of_meetings()] {
        assert_eq!(
            full(&r.body(BodyKind::Sheet), Exactness::Exact).faults(),
            []
        );
        r.faces[0].sense = Sense::Reversed;
        let body = r.body(BodyKind::Sheet);
        let found = full(&body, Exactness::Exact).faults().to_vec();
        assert_eq!(kinds(&found), [FaultKind::LoopSense]);
    }
}

#[test]
fn a_ring_too_thin_for_its_enclosures_fails_closed() {
    // Four planes leaning a millionth of a step inside the sheet's edge at z = 0: each loop's area
    // is decided, and what is left when they are added is far inside their enclosures.
    let far = (1_i64 << 40) - 2;
    let lean = 1 << 20;
    let mut r = Rows::empty();
    let z = r.plane_through(FLOOR);
    let s = r.shell(0);
    let outer =
        [[0, 0], [far, 0], [far, far], [0, far]].map(|q| flat_corner(&mut r, z, q[0], q[1]));
    let west = r.plane_through([[0, 0, 1], [0, 1, 1], [1, 0, 1 - lean]]);
    let east = r.plane_through([[far, 0, -1], [far, 1, -1], [far - 1, 0, lean - 1]]);
    let south = r.plane_through([[0, 0, 1], [1, 0, 1], [0, 1, 1 - lean]]);
    let north = r.plane_through([[0, far, -1], [1, far, -1], [0, far - 1, lean - 1]]);
    let hole = [[west, south], [west, north], [east, north], [east, south]]
        .map(|[x, y]| r.meet([z, x, y]));
    let (outer, hole) = (fins(&mut r, &outer), fins(&mut r, &hole));
    r.face_on(s, &[&outer, &hole], FLOOR);
    let body = r.body(BodyKind::Sheet);
    let found = full(&body, Exactness::Exact).faults().to_vec();
    assert_eq!(kinds(&found), [FaultKind::Unchecked], "{found:?}");
}

#[test]
fn a_small_tilted_box_far_from_the_origin_keeps_its_sign() {
    let far = 1_099_511_627_000_i64;
    let points = corners([far, far, far], [100, 100, 100]);
    let spec = Spec {
        points: &points,
        regions: &[&[&BOX_SHELL]],
    };
    for k in 0..5 {
        let found = faults(&tilted(spec, &jog(31 + k)));
        assert!(found.is_empty(), "{found:?}");
    }
}

#[test]
fn a_seam_between_faces_on_one_plane_passes() {
    // A box whose top is split along x = 50, the two halves sharing the seam.
    let mut points = corners([0, 0, 0], [100, 100, 100]).to_vec();
    points.extend([[50, 0, 100], [50, 100, 100]]);
    let faces: [&[u32]; 7] = [
        &[0, 2, 3, 1],
        &[4, 8, 9, 6],
        &[8, 5, 7, 9],
        &[0, 1, 5, 8, 4],
        &[2, 6, 9, 7, 3],
        &[0, 4, 6, 2],
        &[1, 3, 7, 5],
    ];
    let found = faults(&polyhedron(&points, &faces));
    assert_eq!(found, []);
}

#[test]
fn two_faces_back_to_back_on_one_plane_are_found() {
    // A shell of two triangles sharing all three edges: closed, and enclosing nothing.
    let found = faults(&polyhedron(
        &[[0, 0, 0], [100, 0, 0], [0, 100, 0]],
        &[&[0, 1, 2], &[0, 2, 1]],
    ));
    assert!(kinds(&found).contains(&FaultKind::FacesCross), "{found:?}");
}

#[test]
fn coplanar_faces_apart_pass_and_overlapping_ones_are_found() {
    // Two lumps whose tops lie on one plane: apart, then overlapping, then one inside the other's
    // box in plan, each on a body whose faces are not all convex.
    let lump = |x: i64, side: i64| corners([x, 0, 0], [side, side, 100]);
    let apart = [lump(0, 100), lump(200, 100)].concat();
    let spec = Spec {
        points: &apart,
        regions: TWO.regions,
    };
    assert_eq!(faults(&tilted(spec, &still)), []);
    let overlapping = [lump(0, 100), lump(50, 100)].concat();
    let spec = Spec {
        points: &overlapping,
        regions: TWO.regions,
    };
    let found = faults(&tilted(spec, &still));
    assert!(kinds(&found).contains(&FaultKind::FacesCross), "{found:?}");
    let within = [lump(0, 300), lump(100, 100)].concat();
    let spec = Spec {
        points: &within,
        regions: TWO.regions,
    };
    let found = faults(&tilted(spec, &still));
    assert!(kinds(&found).contains(&FaultKind::FacesCross), "{found:?}");
}

#[test]
fn a_lump_touching_another_at_a_corner_is_found() {
    // The second box's corner rests on the middle of the first's top: no shared vertex.
    let points: Vec<[i64; 3]> = TWO_POINTS
        .iter()
        .enumerate()
        .map(|(k, p)| {
            if k < 8 {
                *p
            } else {
                [p[0] - 250, p[1] + 50, p[2] + 100]
            }
        })
        .collect();
    let spec = Spec {
        points: &points,
        regions: TWO.regions,
    };
    let found = faults(&tilted(spec, &still));
    assert!(kinds(&found).contains(&FaultKind::FacesCross), "{found:?}");
}

#[test]
fn every_face_pair_is_charged_and_a_stop_is_a_stop() {
    let body = tilted(WALL, &jog(37)).body(BodyKind::Solid);
    let mut last = None;
    for work in [1, 10, 100, 400, 1_000, 4_000, 10_000] {
        let mut meter = WorkMeter::new(Budget::of_work(work));
        last = Some(check(&body, Exactness::Exact, Level::Full, &mut meter).is_err());
    }
    assert_eq!(last, Some(false), "the largest budget finishes");
}

#[test]
fn the_same_body_gives_the_same_report() {
    let body = tilted(WALL, &jog(41)).body(BodyKind::Solid);
    assert_eq!(full(&body, Exactness::Exact), full(&body, Exactness::Exact));
}

#[test]
fn a_curve_kind_on_a_plane_based_face_has_no_outline() {
    let mut r = tilted(BOX, &jog(43));
    let e = r.edges[0].curve.row() as usize;
    r.curves[e] = CurveKind::Circle(
        geom_curve::Circle::new(
            [0.0; 3],
            1.0,
            geom_curve::Frame::standard().unwrap(),
            (0.0, 1.0),
        )
        .unwrap(),
    );
    assert_ne!(faults(&r), []);
}

const TETRA_POINTS: [[i64; 3]; 4] = [[0, 0, 0], [300, 0, 0], [0, 300, 0], [0, 0, 300]];
const TETRA_SHELL: [&[&[u32]]; 4] = [&[&[0, 2, 1]], &[&[0, 1, 3]], &[&[0, 3, 2]], &[&[1, 2, 3]]];
const TETRA: Spec<'_> = Spec {
    points: &TETRA_POINTS,
    regions: &[&[&TETRA_SHELL]],
};

/// What a check of [`ring_on_the_grid`] reserves.
const RING_BYTES: u128 = 3_960;

/// What a check of [`ring_of_meetings`] reserves.
const MEETINGS_BYTES: u128 = 9_208;

/// Every budget short of what checking `body` costs stops it, cleanly, and that much does not.
fn stops_wherever_its_budget_ends(body: &Body) {
    let mut whole = WorkMeter::new(Budget::UNLIMITED);
    let report = check(body, Exactness::Exact, Level::Full, &mut whole).unwrap();
    assert_eq!(report.faults(), []);
    let work = u64::try_from(whole.used().work).unwrap();
    for budget in 0..work {
        let mut m = WorkMeter::new(Budget::of_work(budget));
        let stopped = check(body, Exactness::Exact, Level::Full, &mut m);
        assert_eq!(
            stopped.map(|_| ()).map_err(geom_run::Stopped::reason),
            Err(Stop::OverBudget(geom_run::BudgetKind::Work)),
            "{budget}"
        );
    }
    let mut m = WorkMeter::new(Budget::of_work(work));
    assert!(check(body, Exactness::Exact, Level::Full, &mut m).is_ok());
}

#[test]
fn a_check_stops_cleanly_wherever_its_budget_ends() {
    stops_wherever_its_budget_ends(&tilted(TETRA, &jog(53)).body(BodyKind::Solid));
    stops_wherever_its_budget_ends(&ring_on_the_grid().body(BodyKind::Sheet));
    stops_wherever_its_budget_ends(&ring_of_meetings().body(BodyKind::Sheet));
}

#[test]
fn a_meeting_that_cannot_meet_says_so() {
    assert_eq!(
        FaultKind::MeetSingular.what(),
        "the vertex's three planes do not meet in one point"
    );
}

/// `rings` in a sheet of grid corners, each ring a face, edges shared where two rings run along one
/// pair of corners.
fn sheet(rings: &[&[[i64; 3]]]) -> Rows {
    let mut r = Rows::empty();
    let s = r.shell(0);
    let mut seen: Vec<[i64; 3]> = Vec::new();
    for ring in rings {
        let numbers: Vec<u32> = ring
            .iter()
            .map(|p| {
                if let Some(k) = seen.iter().position(|q| q == p) {
                    return k as u32;
                }
                seen.push(*p);
                r.vertex(*p)
            })
            .collect();
        let edges = fins(&mut r, &numbers);
        r.face(s, &[&edges]);
    }
    r
}

/// `rings` as sheets of their own, each a shell, corners shared where two rings have one place.
fn sheets(rings: &[&[[i64; 3]]]) -> Rows {
    let mut r = Rows::empty();
    let mut seen: Vec<[i64; 3]> = Vec::new();
    for ring in rings {
        let s = r.shell(0);
        let numbers: Vec<u32> = ring
            .iter()
            .map(|p| {
                if let Some(k) = seen.iter().position(|q| q == p) {
                    return k as u32;
                }
                seen.push(*p);
                r.vertex(*p)
            })
            .collect();
        let edges = fins(&mut r, &numbers);
        r.face(s, &[&edges]);
    }
    r
}

/// On z = 0: a square right of x = 50, sharing that edge with a face left of it that comes back
/// over the line above it.
const RIGHT: [[i64; 3]; 4] = [[50, 40, 0], [60, 40, 0], [60, 60, 0], [50, 60, 0]];

#[test]
fn two_faces_either_side_of_a_seam_may_return_past_each_other() {
    // The left face crosses the seam's line again higher up, apart from the right face.
    let apart = [
        [0, 40, 0],
        [50, 40, 0],
        [50, 60, 0],
        [40, 60, 0],
        [40, 80, 0],
        [70, 80, 0],
        [70, 100, 0],
        [0, 100, 0],
    ];
    let r = sheet(&[&apart, &RIGHT]);
    assert_eq!(report(&r, BodyKind::Sheet, Exactness::Exact).faults(), []);
    // And from the seam's end, touching the right face at the vertex they share and nowhere else.
    let touching = [
        [0, 40, 0],
        [50, 40, 0],
        [50, 60, 0],
        [70, 80, 0],
        [70, 100, 0],
        [0, 100, 0],
    ];
    let r = sheet(&[&touching, &RIGHT]);
    assert_eq!(report(&r, BodyKind::Sheet, Exactness::Exact).faults(), []);
}

#[test]
fn two_collinear_edges_shared_join_up() {
    // A box with a vertex in the middle of one edge, so two faces share two edges on one line.
    let mut points = BOX_POINTS.to_vec();
    points.push([50, 0, 100]);
    let faces: [&[u32]; 6] = [
        &[0, 2, 3, 1],
        &[4, 8, 5, 7, 6],
        &[0, 1, 5, 8, 4],
        &[2, 6, 7, 3],
        &[0, 4, 6, 2],
        &[1, 3, 7, 5],
    ];
    assert_eq!(faults(&polyhedron(&points, &faces)), []);
}

#[test]
fn two_faces_crossing_from_a_vertex_they_share_are_found() {
    let r = sheets(&[
        &[[0, 0, 0], [100, 0, 0], [100, 100, 0]],
        &[[0, 0, 0], [100, 50, -50], [100, 50, 50]],
    ]);
    let found = report(&r, BodyKind::General, Exactness::Exact);
    assert_eq!(kinds(found.faults()), [FaultKind::FacesCross]);
}

/// The plane z = 0 in doubles that are not grid points, so its plane must come from its corners.
fn off_grid() -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.25, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, 1.0), (0.0, 1.0)),
        )
        .unwrap(),
    )
}

#[test]
fn a_bent_face_on_doubles_stands_for_its_fan() {
    let mut r = sheet(&[&[[0, 0, 0], [10, 0, 0], [10, 10, 1], [0, 10, 0]]]);
    r.surfaces[0] = off_grid();
    assert_eq!(report(&r, BodyKind::Sheet, certified()).faults(), []);
}

#[test]
fn a_flat_face_is_tested_against_each_triangle_of_a_fan_on_its_plane() {
    // The fan's first triangle lies on z = 0 with the small square beside its diagonal.
    let mut r = sheets(&[
        &[[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 1]],
        &[[1, 8, 0], [3, 8, 0], [3, 9, 0], [1, 9, 0]],
    ]);
    r.surfaces[0] = level(0.0);
    assert_eq!(report(&r, BodyKind::General, certified()).faults(), []);
}

#[test]
fn a_bent_face_with_a_straight_corner_has_no_fan() {
    let mut r = sheets(&[
        &[[0, 0, 0], [5, 0, 0], [10, 0, 0], [10, 10, 1], [0, 10, 0]],
        &[[20, 0, -5], [20, 10, -5], [20, 10, 5], [20, 0, 5]],
    ]);
    r.surfaces[0] = level(0.0);
    let found = report(&r, BodyKind::General, certified());
    assert_eq!(kinds(found.faults()), [FaultKind::PairUnchecked]);
}

#[test]
fn a_flat_face_against_one_with_no_outline_fails_closed() {
    let mut r = sheets(&[&[[5, 5, 0], [9, 5, 0], [9, 9, 0], [5, 9, 0]]]);
    let e = r.edge(None);
    let s = r.shell(0);
    r.face(s, &[&[(e, Sense::Forward)]]);
    let found = report(&r, BodyKind::General, certified());
    assert_eq!(kinds(found.faults()), [FaultKind::PairUnchecked]);
}

/// Adds a box of corners `points`, facing outward, as a solid region of its own; its grid corners.
fn add_lump(r: &mut Rows, points: &[[i64; 3]; 8]) {
    let solid = r.region(RegionKind::Solid);
    let s = r.shell(solid);
    let base = r.vertices.len() as u32;
    for p in points {
        r.vertex(*p);
    }
    let rings: Vec<Vec<u32>> = BOX_SHELL
        .iter()
        .map(|f| f[0].iter().map(|v| v + base).collect())
        .collect();
    let refs: Vec<&[u32]> = rings.iter().map(Vec::as_slice).collect();
    faces_into(r, s, &refs);
}

#[test]
fn a_lump_of_grid_points_inside_a_shell_of_meetings_is_found() {
    let big = corners([0, 0, 0], [900, 900, 900]);
    let spec = Spec {
        points: &big,
        regions: &[&[&BOX_SHELL]],
    };
    let mut r = tilted(spec, &jog(47));
    add_lump(&mut r, &corners([300, 300, 300], [100, 100, 100]));
    assert_eq!(kinds(&faults(&r)), [FaultKind::ShellNesting]);
}

#[test]
fn a_ray_along_a_seam_counts_one_of_its_two_faces() {
    // A box whose top is split along y = 50, one corner a meeting of planes, and inside it a lump
    // with a corner under the seam, so the ray meets the seam exactly.
    let mut points = BOX_POINTS.to_vec();
    points.extend([[0, 50, 100], [100, 50, 100]]);
    let faces: [&[u32]; 7] = [
        &[0, 2, 3, 1],
        &[4, 5, 9, 8],
        &[8, 9, 7, 6],
        &[0, 1, 5, 4],
        &[2, 6, 7, 3],
        &[0, 4, 8, 6, 2],
        &[1, 3, 7, 9, 5],
    ];
    let mut r = polyhedron(&points, &faces);
    let x = r.plane_through([[100, 0, 0], [100, 1, 0], [100, 0, 1]]);
    let y = r.plane_through([[0, 100, 0], [0, 100, 1], [1, 100, 0]]);
    let z = r.plane_through([[0, 0, 100], [1, 0, 100], [0, 1, 100]]);
    r.meets.push((7, [x, y, z]));
    add_lump(&mut r, &corners([30, 50, 30], [10, 10, 10]));
    assert_eq!(kinds(&faults(&r)), [FaultKind::ShellNesting]);
}

#[test]
fn two_lumps_sharing_a_meeting_are_judged_by_their_other_corners() {
    let mut r = Rows::empty();
    let x = r.plane_through([[0, 0, 0], [0, 1, 0], [0, 0, 1]]);
    let y = r.plane_through([[0, 0, 0], [0, 0, 1], [1, 0, 0]]);
    let z = r.plane_through([[0, 0, 0], [1, 0, 0], [0, 1, 0]]);
    let apex = r.meet([x, y, z]);
    assert_eq!(apex, 0);
    for p in [
        [100, 0, 0],
        [0, 100, 0],
        [0, 0, 100],
        [-100, 0, 0],
        [0, -100, 0],
        [0, 0, -100],
    ] {
        r.vertex(p);
    }
    let up = r.region(RegionKind::Solid);
    let down = r.region(RegionKind::Solid);
    let (s, t) = (r.shell(up), r.shell(down));
    faces_into(&mut r, s, &[&[0, 2, 1], &[0, 1, 3], &[0, 3, 2], &[1, 2, 3]]);
    faces_into(&mut r, t, &[&[0, 4, 5], &[0, 6, 4], &[0, 5, 6], &[4, 6, 5]]);
    assert_eq!(faults(&r), []);
}

#[test]
fn a_shell_of_meetings_inside_a_bent_one_fails_closed() {
    let mut r = tilted(BOX, &jog(59));
    let mut bent = corners([-500, -500, -500], [1500, 1500, 1500]);
    bent[7][2] += 1;
    add_lump(&mut r, &bent);
    let certified = Exactness::Certified {
        bound: Steps(2),
        step: GridStep::Q0,
    };
    let found = report(&r, BodyKind::Solid, certified);
    assert_eq!(kinds(found.faults()), [FaultKind::PairUnchecked]);
}

#[test]
fn a_shell_of_meetings_with_every_corner_on_another_fails_closed() {
    // A cube dented on all six sides, and a cavity between four of the dents' tips, one tip a
    // meeting of planes, so no corner of the cavity is off the cube.
    let mut r = Rows::empty();
    let region = r.region(RegionKind::Solid);
    let outer = r.shell(region);
    for c in corners([0, 0, 0], [12, 12, 12]) {
        r.vertex(c);
    }
    for t in [
        [6, 6, 2],
        [6, 6, 10],
        [6, 2, 6],
        [6, 10, 6],
        [2, 6, 6],
        [10, 6, 6],
    ] {
        r.vertex(t);
    }
    let mut rings: Vec<Vec<u32>> = Vec::new();
    for (f, ring) in CUBE_FACES.iter().enumerate() {
        for k in 0..4 {
            rings.push(vec![ring[k], ring[(k + 1) % 4], 8 + f as u32]);
        }
    }
    let refs: Vec<&[u32]> = rings.iter().map(Vec::as_slice).collect();
    faces_into(&mut r, outer, &refs);
    let cavity = r.shell(region);
    faces_into(
        &mut r,
        cavity,
        &[&[8, 10, 9], &[8, 9, 12], &[8, 12, 10], &[9, 10, 12]],
    );
    let x = r.plane_through([[6, 0, 0], [6, 1, 0], [6, 0, 1]]);
    let y = r.plane_through([[0, 6, 0], [0, 6, 1], [1, 6, 0]]);
    let z = r.plane_through([[0, 0, 2], [1, 0, 2], [0, 1, 2]]);
    r.meets.push((8, [x, y, z]));
    assert_eq!(kinds(&faults(&r)), [FaultKind::PairUnchecked]);
}

/// The largest grid coordinate.
const FAR: i64 = 1_099_511_627_775;

#[test]
fn a_flat_face_at_each_edge_of_the_grid_has_its_sides() {
    // No neighbours, so each side's plane is stood on it, a step towards the origin.
    for z in [FAR, -FAR] {
        let inward = if z > 0 { z - 10 } else { z + 5 };
        let r = sheets(&[
            &[[0, 0, z], [10, 0, z], [10, 10, z], [0, 10, z]],
            &[
                [20, 0, inward],
                [20, 10, inward],
                [20, 10, inward + 5],
                [20, 0, inward + 5],
            ],
        ]);
        let found = report(&r, BodyKind::General, Exactness::Exact);
        assert_eq!(found.faults(), []);
    }
}

#[test]
fn a_vertex_on_four_planes_keeps_three_and_its_edges_find_the_fourth() {
    // An octahedron, four faces at each vertex. An edge's second plane is in neither end's three,
    // so it can only come from the face across the edge.
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let m = 100;
    let plane = |r: &mut Rows, x: i64, y: i64, z: i64| {
        r.plane_through([[x * m, 0, 0], [0, y * m, 0], [0, 0, z * m]])
    };
    let mut planes = Vec::new();
    for (x, y, z) in [
        (1, 1, 1),
        (1, 1, -1),
        (1, -1, 1),
        (1, -1, -1),
        (-1, 1, 1),
        (-1, 1, -1),
        (-1, -1, 1),
        (-1, -1, -1),
    ] {
        planes.push(plane(&mut r, x, y, z));
    }
    for [a, b, c] in [
        [0, 2, 3],
        [0, 4, 5],
        [0, 2, 4],
        [4, 5, 6],
        [2, 3, 6],
        [1, 3, 5],
    ] {
        r.meet([planes[a], planes[b], planes[c]]);
    }
    let s = r.shell(solid);
    faces_into(
        &mut r,
        s,
        &[
            &[0, 1, 2],
            &[0, 5, 1],
            &[0, 2, 4],
            &[0, 4, 5],
            &[3, 2, 1],
            &[3, 1, 5],
            &[3, 4, 2],
            &[3, 5, 4],
        ],
    );
    assert_eq!(faults(&r), []);
}

#[test]
fn a_face_with_a_hole_on_doubles_takes_its_plane_from_its_corners() {
    let mut r = Rows::empty();
    let (s, t) = (r.shell(0), r.shell(0));
    for p in [
        [0, 0, 0],
        [100, 0, 0],
        [100, 100, 0],
        [0, 100, 0],
        [40, 40, 0],
        [40, 60, 0],
        [60, 60, 0],
        [60, 40, 0],
        [200, 0, -50],
        [200, 100, -50],
        [200, 100, 50],
        [200, 0, 50],
    ] {
        r.vertex(p);
    }
    let (outer, hole) = (fins(&mut r, &[0, 1, 2, 3]), fins(&mut r, &[4, 5, 6, 7]));
    r.face(s, &[&outer, &hole]);
    let other = fins(&mut r, &[8, 9, 10, 11]);
    r.face(t, &[&other]);
    r.surfaces[0] = off_grid();
    assert_eq!(report(&r, BodyKind::General, certified()).faults(), []);
}

#[test]
fn a_box_with_a_corner_lifted_is_a_fan_on_each_bent_face() {
    let mut points = BOX_POINTS.to_vec();
    points[7][2] += 1;
    let faces: Vec<&[u32]> = BOX_SHELL.iter().map(|f| f[0]).collect();
    let r = polyhedron(&points, &faces);
    let room = Exactness::Certified {
        bound: Steps(2),
        step: GridStep::Q0,
    };
    assert_eq!(report(&r, BodyKind::Solid, room).faults(), []);
}
